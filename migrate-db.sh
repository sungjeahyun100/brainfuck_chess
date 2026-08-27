#!/usr/bin/env bash
# Explicit split-schema database release runner. This script never deploys the app.

set -Eeuo pipefail

PROJECT_ID="var-chess-bfc"
INSTANCE_NAME="deck-chess-postgres"
DATABASE_NAME="deck_chess"

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

if [[ -z "${ADMIN_DATABASE_URL:-}" ]]; then
  echo "ADMIN_DATABASE_URL is required and must identify an administrative login." >&2
  echo "Do not use either application runtime DATABASE_URL." >&2
  exit 2
fi
if [[ "$ADMIN_DATABASE_URL" =~ ^postgres(ql)?://[^/@:]+:[^/@]+@ ]] \
   || [[ "$ADMIN_DATABASE_URL" =~ (^|[[:space:]])password[[:space:]]*= ]]; then
  echo "ADMIN_DATABASE_URL must not contain a password." >&2
  echo "Use .pgpass, PGPASSFILE, PGPASSWORD, or an approved credential mechanism." >&2
  exit 2
fi

PSQL_BIN="${PSQL_BIN:-psql}"
if ! command -v "$PSQL_BIN" >/dev/null 2>&1; then
  echo "psql executable not found: $PSQL_BIN" >&2
  exit 2
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

SHARED_MIGRATIONS=(
  "$SCRIPT_DIR/server/db/shared/20260826000000_profile_visibility.sql"
)
PROD_MIGRATIONS=(
  "$SCRIPT_DIR/server/db/prod/20260826000500_create_game_records.sql"
  "$SCRIPT_DIR/server/db/prod/20260826001000_game_record_ownership.sql"
  "$SCRIPT_DIR/server/db/prod/20260827000000_challenge_clears.sql"
)
TEST_MIGRATIONS=(
  "$SCRIPT_DIR/server/db/test/20260826000500_create_game_records.sql"
  "$SCRIPT_DIR/server/db/test/20260826001000_game_record_ownership.sql"
  "$SCRIPT_DIR/server/db/test/20260827000000_challenge_clears.sql"
)

if [[ "$TARGET" == "prod" ]]; then
  TARGET_MIGRATIONS=("${PROD_MIGRATIONS[@]}")
  POSTFLIGHT="$SCRIPT_DIR/server/db/admin/verify_prod_contract.sql"
else
  TARGET_MIGRATIONS=("${TEST_MIGRATIONS[@]}")
  POSTFLIGHT="$SCRIPT_DIR/server/db/admin/verify_test_contract.sql"
fi

for migration in "${SHARED_MIGRATIONS[@]}" "${TARGET_MIGRATIONS[@]}" "$POSTFLIGHT"; do
  if [[ ! -f "$migration" ]]; then
    echo "Required database release file is missing: $migration" >&2
    exit 2
  fi
done

echo "Database migration target"
echo "  target:   $TARGET"
echo "  project:  $PROJECT_ID"
echo "  instance: $INSTANCE_NAME"
echo "  database: $DATABASE_NAME"
echo
echo "Ordered release SQL"
for migration in "${SHARED_MIGRATIONS[@]}"; do
  echo "  [shared] $(basename -- "$migration")"
done
for migration in "${TARGET_MIGRATIONS[@]}"; do
  echo "  [$TARGET]   $(basename -- "$migration")"
done

if [[ "$TARGET" == "prod" ]]; then
  echo
  echo "WARNING: confirm that a current Cloud SQL backup exists before continuing."
  echo "This command changes the production database; deployment remains a separate action."
  echo
  confirmation=""
  if ! read -r -p 'Continue only by typing exactly "prod": ' confirmation; then
    echo "Production migration cancelled because confirmation input was unavailable." >&2
    exit 1
  fi
  if [[ "$confirmation" != "prod" ]]; then
    echo "Production migration cancelled; no database command was executed." >&2
    exit 1
  fi
fi

run_sql_file() {
  local sql_file="$1"
  "$PSQL_BIN" \
    -X \
    --no-psqlrc \
    --dbname "$ADMIN_DATABASE_URL" \
    --set=ON_ERROR_STOP=1 \
    --file "$sql_file"
}

echo
echo "== Preflight =="
run_sql_file "$SCRIPT_DIR/server/db/admin/preflight.sql"

echo
echo "== Applying ordered release SQL =="
for migration in "${SHARED_MIGRATIONS[@]}"; do
  echo "[shared] $(basename -- "$migration")"
  run_sql_file "$migration"
done
for migration in "${TARGET_MIGRATIONS[@]}"; do
  echo "[$TARGET] $(basename -- "$migration")"
  run_sql_file "$migration"
done

echo
echo "== Runtime contract =="
run_sql_file "$POSTFLIGHT"

if [[ "$VERIFY_ISOLATION" == true ]]; then
  echo
  echo "== Rollback-only environment isolation probe =="
  run_sql_file "$SCRIPT_DIR/server/db/admin/verify_environment_isolation.sql"
fi

echo
echo "$TARGET database release and contract verification completed successfully."
echo "No application deployment was performed."
