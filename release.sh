#!/usr/bin/env bash
# Repository-tracked release orchestrator. Credentials are supplied only via environment variables.

set -Eeuo pipefail

PROJECT_ID="var-chess-bfc"
INSTANCE_NAME="deck-chess-postgres"
DATABASE_NAME="deck_chess"
PROXY_HOST="127.0.0.1"
PROXY_PORT="5432"

TARGET="${1:-}"
VERIFY_ISOLATION=false

if [[ "$TARGET" != "prod" && "$TARGET" != "test" ]]; then
  echo "Usage: $0 [prod|test] [--verify-isolation]" >&2
  exit 2
fi

shift
if [[ "${1:-}" == "--verify-isolation" ]]; then
  VERIFY_ISOLATION=true
  shift
fi
if (( $# != 0 )); then
  echo "Usage: $0 [prod|test] [--verify-isolation]" >&2
  exit 2
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
MIGRATION_RUNNER="$SCRIPT_DIR/migrate-db.sh"
DEPLOY_RUNNER="$SCRIPT_DIR/reinstall-image.sh"

for required_script in "$MIGRATION_RUNNER" "$DEPLOY_RUNNER"; do
  if [[ ! -x "$required_script" ]]; then
    echo "Required executable script is missing: $required_script" >&2
    exit 2
  fi
done

if [[ -z "${ADMIN_DATABASE_URL:-}" ]]; then
  echo "ADMIN_DATABASE_URL is required and must use an administrative login." >&2
  exit 2
fi

if [[ "$TARGET" == "prod" ]]; then
  EXPECTED_BRANCH="main"
else
  EXPECTED_BRANCH="develop"
fi
CURRENT_BRANCH="$(git -C "$SCRIPT_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
if [[ "$CURRENT_BRANCH" != "$EXPECTED_BRANCH" ]]; then
  echo "Current branch:  $CURRENT_BRANCH" >&2
  echo "Expected branch: $EXPECTED_BRANCH" >&2
  echo "Release cancelled before backup or database migration." >&2
  exit 1
fi
if [[ -n "$(git -C "$SCRIPT_DIR" status --porcelain --untracked-files=normal)" ]]; then
  echo "Release requires a clean working tree containing the reviewed migration and deploy scripts." >&2
  echo "Commit the intended release changes before retrying." >&2
  exit 1
fi

MIGRATION_ARGS=("$TARGET")
if [[ "$VERIFY_ISOLATION" == true ]]; then
  MIGRATION_ARGS+=("--verify-isolation")
fi

PROXY_STARTED=false
PROXY_PID=""
PROXY_LOG=""

cleanup_proxy() {
  if [[ "$PROXY_STARTED" == true && -n "$PROXY_PID" ]]; then
    kill "$PROXY_PID" 2>/dev/null || true
    wait "$PROXY_PID" 2>/dev/null || true
  fi
  if [[ -n "$PROXY_LOG" ]]; then
    rm -f -- "$PROXY_LOG"
  fi
}
trap cleanup_proxy EXIT

ensure_local_proxy() {
  if [[ "$ADMIN_DATABASE_URL" != postgresql://*"@$PROXY_HOST:$PROXY_PORT/"* \
        && "$ADMIN_DATABASE_URL" != postgres://*"@$PROXY_HOST:$PROXY_PORT/"* \
        && "$ADMIN_DATABASE_URL" != postgresql://*"@localhost:$PROXY_PORT/"* \
        && "$ADMIN_DATABASE_URL" != postgres://*"@localhost:$PROXY_PORT/"* ]]; then
    echo "ADMIN_DATABASE_URL does not use the managed local proxy endpoint; using it as provided."
    return
  fi

  if command -v pg_isready >/dev/null 2>&1 \
     && pg_isready --host="$PROXY_HOST" --port="$PROXY_PORT" --dbname="$DATABASE_NAME" >/dev/null 2>&1; then
    echo "Reusing PostgreSQL endpoint already listening on $PROXY_HOST:$PROXY_PORT."
    return
  fi

  if ! command -v cloud-sql-proxy >/dev/null 2>&1; then
    echo "cloud-sql-proxy is required for the configured local ADMIN_DATABASE_URL." >&2
    exit 2
  fi
  if ! command -v gcloud >/dev/null 2>&1; then
    echo "gcloud is required to resolve the Cloud SQL connection name." >&2
    exit 2
  fi

  local connection_name
  connection_name="$(gcloud sql instances describe "$INSTANCE_NAME" \
    --project="$PROJECT_ID" \
    --format='value(connectionName)')"
  if [[ -z "$connection_name" ]]; then
    echo "Could not resolve the Cloud SQL instance connection name." >&2
    exit 1
  fi

  PROXY_LOG="$(mktemp)"
  cloud-sql-proxy \
    --address="$PROXY_HOST" \
    --port="$PROXY_PORT" \
    "$connection_name" >"$PROXY_LOG" 2>&1 &
  PROXY_PID="$!"
  PROXY_STARTED=true

  local attempt
  for attempt in {1..20}; do
    if command -v pg_isready >/dev/null 2>&1 \
       && pg_isready --host="$PROXY_HOST" --port="$PROXY_PORT" --dbname="$DATABASE_NAME" >/dev/null 2>&1; then
      echo "Cloud SQL Auth Proxy ready on $PROXY_HOST:$PROXY_PORT."
      return
    fi
    if ! kill -0 "$PROXY_PID" 2>/dev/null; then
      echo "Cloud SQL Auth Proxy exited before becoming ready:" >&2
      sed -n '1,80p' "$PROXY_LOG" >&2
      exit 1
    fi
    sleep 0.5
  done

  echo "Cloud SQL Auth Proxy did not become ready within 10 seconds:" >&2
  sed -n '1,80p' "$PROXY_LOG" >&2
  exit 1
}

echo "Release target"
echo "  target:   $TARGET"
echo "  project:  $PROJECT_ID"
echo "  instance: $INSTANCE_NAME"
echo "  database: $DATABASE_NAME"
echo

if [[ "$TARGET" == "prod" ]]; then
  if ! command -v gcloud >/dev/null 2>&1; then
    echo "gcloud is required to create the production backup." >&2
    exit 2
  fi

  confirmation=""
  if ! read -r -p 'Run backup, production migration, and deployment by typing exactly "release-prod": ' confirmation; then
    echo "Production release cancelled because confirmation input was unavailable." >&2
    exit 1
  fi
  if [[ "$confirmation" != "release-prod" ]]; then
    echo "Production release cancelled; backup, migration, and deployment were not started." >&2
    exit 1
  fi

  echo
  echo "== Cloud SQL Auth Proxy =="
  ensure_local_proxy

  echo
  echo "== Cloud SQL production backup =="
  gcloud sql backups create \
    --instance="$INSTANCE_NAME" \
    --project="$PROJECT_ID"

  echo
  echo "== Production database migration and contract =="
  # The stronger release-prod confirmation above is the explicit production
  # opt-in for this wrapper. Direct migrate-db.sh use still requires "prod".
  printf 'prod\n' | "$MIGRATION_RUNNER" "${MIGRATION_ARGS[@]}"
else
  echo "== Cloud SQL Auth Proxy =="
  ensure_local_proxy
  echo
  echo "== Test database migration and contract =="
  "$MIGRATION_RUNNER" "${MIGRATION_ARGS[@]}"
fi

echo
echo "== Application image build and deploy =="
"$DEPLOY_RUNNER" "$TARGET"

echo
echo "$TARGET backup/migration/deployment release completed successfully."
