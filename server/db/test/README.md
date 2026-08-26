# Test data migrations

Test-only migrations for server-side decks, custom pieces and images belong here.
They may be applied independently of production. A test deployment must not run
files from `../shared`, `../prod`, or `../admin`.

After the shared profile-visibility release, apply only these test files in
order:

1. `20260826000500_create_game_records.sql`
2. `20260826001000_game_record_ownership.sql`

The first grants `test_app` only SELECT/INSERT/UPDATE on `test.game_records`; the
second backfills legacy test rows. Neither script reads or writes `prod`.

Both scripts expect a clean administrative session with no permanent
`deck_chess` or `deck_chess_schema_owner` membership. Required owner roles are
granted transactionally and removed before commit, so rerunning the ordered
scripts is safe after a successful release.

Run the supported workflow as `./migrate-db.sh test` from the repository root.
The runner uses an explicit test-only manifest and verifies the
`deck_chess_test` runtime contract afterward. See `../RELEASE.md`.
