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

`test_migrate_db.sh` is a no-database control-flow test for the root runner. Its
fake `psql` verifies explicit prod/test manifests, production confirmation before
any connection, and immediate stop after a SQL-file failure. It does not replace
the disposable PostgreSQL fixture above.

`run_migration_regression.sh` automates the full sequence against a brand-new
disposable PostgreSQL cluster whose database is named `deck_chess`:

```sh
export DISPOSABLE_ADMIN_DATABASE_URL='postgresql://postgres@127.0.0.1:5432/deck_chess'
./server/db/testing/run_migration_regression.sh
```

It provisions the fixture, runs fresh test and prod releases, runs both a second
time, executes the rollback-only isolation probe, and finishes with the ownership
verification. The fixture creates cluster-level roles, so never use an existing
or shared PostgreSQL cluster.
