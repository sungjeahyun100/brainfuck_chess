#!/usr/bin/env bash

set -Eeuo pipefail

REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
PSQL_BIN="${PSQL_BIN:-psql}"

if [[ -z "${DISPOSABLE_ADMIN_DATABASE_URL:-}" ]]; then
  echo "DISPOSABLE_ADMIN_DATABASE_URL must identify a new disposable deck_chess database." >&2
  exit 2
fi
if ! command -v "$PSQL_BIN" >/dev/null 2>&1; then
  echo "psql executable not found: $PSQL_BIN" >&2
  exit 2
fi

run_fixture_sql() {
  local sql_file="$1"
  "$PSQL_BIN" \
    -X --no-psqlrc --dbname "$DISPOSABLE_ADMIN_DATABASE_URL" \
    --set=ON_ERROR_STOP=1 --file "$sql_file"
}

run_migration() {
  ADMIN_DATABASE_URL="$DISPOSABLE_ADMIN_DATABASE_URL" \
    PSQL_BIN="$PSQL_BIN" \
    "$REPOSITORY_ROOT/migrate-db.sh" "$@"
}

run_fixture_sql "$REPOSITORY_ROOT/server/db/testing/ownership_fixture.sql"

# Verify the original ownership contract before retention adds DELETE grants.
for pass in 1 2; do
  run_fixture_sql "$REPOSITORY_ROOT/server/db/shared/20260826000000_profile_visibility.sql"
  for environment in test prod; do
    run_fixture_sql "$REPOSITORY_ROOT/server/db/$environment/20260826000500_create_game_records.sql"
    run_fixture_sql "$REPOSITORY_ROOT/server/db/$environment/20260826001000_game_record_ownership.sql"
  done
done
run_fixture_sql "$REPOSITORY_ROOT/server/db/testing/verify_ownership_migrations.sql"

run_migration test
printf 'prod\n' | run_migration prod

# A second complete pass proves that shared and environment-specific releases
# are idempotent. The explicit test pass also exercises the rollback-only probe.
run_migration test --verify-isolation
printf 'prod\n' | run_migration prod

run_fixture_sql "$REPOSITORY_ROOT/server/db/testing/verify_account_decks.sql"

echo 'Disposable PostgreSQL migration regression PASS'
