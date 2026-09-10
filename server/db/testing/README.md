# Disposable ownership regression fixture

These SQL files are test fixtures, not release migrations. Use them only in a
new disposable PostgreSQL database whose roles and schemas may be created from
scratch.

1. Run `ownership_fixture.sql` as the disposable database administrator.
2. Run the shared, test and prod release SQL in their documented order.
3. Run the same release SQL a second time to exercise idempotency.
4. Run `verify_ownership_migrations.sql` as the same administrator.

The verification checks direct temporary-role cleanup, runtime schema
isolation, application DML grants, exactly two user FKs per game-record table,
FK enforcement, and cross-environment denial. Never point this fixture at Cloud
SQL, a developer database, or any database containing user data.

For account decks, apply the current release SQL (including
`20260908000000_account_decks.sql`) twice, run both current runtime contracts,
and then run `verify_account_decks.sql`. The latter uses a rolled-back fixture
and exercises runtime CRUD, owner FK enforcement, and bidirectional isolation.
The older `verify_ownership_migrations.sql` expects the pre-retention game-record
permissions; run it before the analysis/retention release that intentionally adds
DELETE. It is not a postflight for the current complete migration set.

The Rust account-deck integration test requires `TEST_DECK_DATABASE_URL` pointing
to this disposable migrated database:

```sh
cargo test -p brainfuck-chess-server deck::tests::postgres_ -- --ignored
```

For G3-C exact analysis Draw persistence, apply
`20260910000000_analysis_draws.sql` in the matching environment release sequence
(`migrate-db.sh` includes it). The server and runtime postflight require the
`game_analysis_nodes.draws` JSONB column. The disposable fixture's `fixture-user`
is used by this opt-in repository integration test:

```sh
cargo test --offline -p brainfuck-chess-server postgres_analysis_draws_roundtrip_and_concurrent_idempotency -- --ignored
```

Set `TEST_ANALYSIS_DATABASE_URL` to the disposable migrated test database first.
The test creates a unique record and exercises concurrent create/append retries,
SQL Draw round-trip, and validation using a freshly constructed repository.
It removes its record on success; failed fixtures belong only in disposable DBs.
