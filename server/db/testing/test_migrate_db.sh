#!/usr/bin/env bash

set -Eeuo pipefail

REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
TEST_TMPDIR="$(mktemp -d)"
trap 'rm -rf -- "$TEST_TMPDIR"' EXIT

FAKE_PSQL="$TEST_TMPDIR/psql"
FAKE_LOG="$TEST_TMPDIR/psql.log"

cat >"$FAKE_PSQL" <<'FAKE'
#!/usr/bin/env bash
set -Eeuo pipefail
sql_file=""
while (( $# > 0 )); do
  if [[ "$1" == "--file" ]]; then
    sql_file="$2"
    shift 2
  else
    shift
  fi
done
printf '%s\n' "$sql_file" >>"$FAKE_PSQL_LOG"
if [[ -n "${FAKE_PSQL_FAIL_FILE:-}" && "$(basename -- "$sql_file")" == "$FAKE_PSQL_FAIL_FILE" ]]; then
  exit 9
fi
FAKE
chmod +x "$FAKE_PSQL"

run_runner() {
  ADMIN_DATABASE_URL='postgresql://admin@invalid/deck_chess' \
    PSQL_BIN="$FAKE_PSQL" \
    FAKE_PSQL_LOG="$FAKE_LOG" \
    "$REPOSITORY_ROOT/migrate-db.sh" "$@"
}

assert_contains_path() {
  local expected="$1"
  while IFS= read -r actual; do
    [[ "$actual" == *"$expected" ]] && return 0
  done <"$FAKE_LOG"
  echo "Expected psql invocation containing: $expected" >&2
  exit 1
}

assert_excludes_path() {
  local forbidden="$1"
  while IFS= read -r actual; do
    if [[ "$actual" == *"$forbidden"* ]]; then
      echo "Unexpected psql invocation containing: $forbidden" >&2
      exit 1
    fi
  done <"$FAKE_LOG"
}

assert_before() {
  local first="$1"
  local second="$2"
  local line_number=0
  local first_line=0
  local second_line=0
  while IFS= read -r actual; do
    ((line_number += 1))
    [[ "$actual" == *"$first"* ]] && first_line="$line_number"
    [[ "$actual" == *"$second"* ]] && second_line="$line_number"
  done <"$FAKE_LOG"
  if (( first_line == 0 || second_line == 0 || first_line >= second_line )); then
    echo "Expected $first before $second" >&2
    exit 1
  fi
}

: >"$FAKE_LOG"
run_runner test >/dev/null
assert_contains_path '/server/db/shared/20260826000000_profile_visibility.sql'
assert_contains_path '/server/db/test/20260826000500_create_game_records.sql'
assert_contains_path '/server/db/test/20260826001000_game_record_ownership.sql'
assert_contains_path '/server/db/admin/verify_test_contract.sql'
assert_excludes_path '/server/db/prod/'
assert_before '/server/db/shared/' '/server/db/test/20260826000500_create_game_records.sql'
assert_before '20260826000500_create_game_records.sql' '20260826001000_game_record_ownership.sql'
assert_before '20260826001000_game_record_ownership.sql' 'verify_test_contract.sql'

: >"$FAKE_LOG"
printf 'prod\n' | run_runner prod >/dev/null
assert_contains_path '/server/db/prod/20260826000500_create_game_records.sql'
assert_contains_path '/server/db/prod/20260826001000_game_record_ownership.sql'
assert_contains_path '/server/db/admin/verify_prod_contract.sql'
assert_excludes_path '/server/db/test/'
assert_before '/server/db/shared/' '/server/db/prod/20260826000500_create_game_records.sql'
assert_before '20260826000500_create_game_records.sql' '20260826001000_game_record_ownership.sql'
assert_before '20260826001000_game_record_ownership.sql' 'verify_prod_contract.sql'

: >"$FAKE_LOG"
run_runner test --verify-isolation >/dev/null
assert_before 'verify_test_contract.sql' 'run_environment_isolation_probe.sql'

: >"$FAKE_LOG"
if printf 'yes\n' | run_runner prod >/dev/null 2>&1; then
  echo 'Production confirmation unexpectedly accepted a non-prod value' >&2
  exit 1
fi
if [[ -s "$FAKE_LOG" ]]; then
  echo 'Production cancellation executed a database command' >&2
  exit 1
fi

: >"$FAKE_LOG"
if ADMIN_DATABASE_URL='postgresql://admin@invalid/deck_chess' \
     PSQL_BIN="$FAKE_PSQL" \
     FAKE_PSQL_LOG="$FAKE_LOG" \
     FAKE_PSQL_FAIL_FILE='20260826000500_create_game_records.sql' \
     "$REPOSITORY_ROOT/migrate-db.sh" test >/dev/null 2>&1; then
  echo 'Runner ignored a migration failure' >&2
  exit 1
fi
assert_excludes_path '/server/db/test/20260826001000_game_record_ownership.sql'
assert_excludes_path '/server/db/admin/verify_test_contract.sql'

echo 'migrate-db.sh control-flow tests PASS'
