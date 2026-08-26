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
