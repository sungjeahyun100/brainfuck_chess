# Test data migrations

Test-only migrations for server-side decks, custom pieces and images belong here.
They may be applied independently of production. A test deployment must not run
files from `../shared`, `../prod`, or `../admin`.

Apply `20260826001000_game_record_ownership.sql` independently to the test schema
after the shared profile-visibility release. It never reads or writes `prod`.
