# Split-schema database release workflow

`migrate-db.sh` is the explicit administrative release path for the ordered SQL
in `shared`, `prod`, and `test`. It does not use the checksum-preserved legacy
SQLx history in `server/migrations`, start an application, build an image, or
deploy Cloud Run. Application startup and `reinstall-image.sh` never run this
workflow.

For a local one-command orchestration, use the intentionally gitignored root
`release.sh`. It preserves the same boundaries internally: production backup
must succeed, then the migration contract must pass, and only then is
`reinstall-image.sh` invoked. A failure stops the remaining steps.

## Administrative connection

Connect through an operator-controlled Cloud SQL Auth Proxy (or an equivalent
approved private connection) and provide a distinct administrative libpq URI or
conninfo string in `ADMIN_DATABASE_URL`. The login must be an administrator such
as `postgres`, not `deck_chess` or `deck_chess_test`.

For example, after starting the proxy separately:

```sh
export ADMIN_DATABASE_URL='postgresql://postgres@127.0.0.1:5432/deck_chess'
./migrate-db.sh test
```

Use `.pgpass`, `PGPASSWORD`, or the proxy/organization's approved password
mechanism. Do not put a password in this repository, reuse an application
`DATABASE_URL`, or print the value. The runner passes the connection value to
libpq through the environment rather than the process command line.

The runner always identifies and verifies the fixed target:

- project: `var-chess-bfc`
- instance: `deck-chess-postgres`
- database: `deck_chess`

Preflight rejects a different database, either runtime login, a session already
using another role, missing split-schema prerequisites, or permanent
`deck_chess`/`deck_chess_schema_owner` membership on the administrator.

## Test release

One-command local workflow:

```sh
./release.sh test
```

Equivalent manual workflow:

1. Optionally create or confirm a test backup.
2. Run `./migrate-db.sh test`.
3. Require the test database contract to report `PASS`.
4. Deploy test with `./reinstall-image.sh test`.
5. Run the application smoke test.

The test command applies only the shared manifest followed by the explicit
`server/db/test` manifest. It never runs production SQL.

## Production release

Create and confirm a current Cloud SQL backup first:

```sh
gcloud sql backups create \
  --instance=deck-chess-postgres \
  --project=var-chess-bfc
```

Then perform the release as distinct operator actions:

1. Run `./migrate-db.sh prod`.
2. Review the displayed project, instance, database and ordered files.
3. At the prompt, type exactly `prod` to opt in.
4. Require the production runtime contract and membership cleanup to report
   `PASS`.
5. Deploy with `./reinstall-image.sh prod`.
6. Run the production smoke test.

The equivalent one-command local workflow is:

```sh
./release.sh prod
```

It requires the exact confirmation `release-prod`, creates the Cloud SQL backup,
runs the production migration and contract, and deploys only after both succeed.
The optional deeper check is `./release.sh prod --verify-isolation`.

The production manifest is fixed to this order:

1. `shared/20260826000000_profile_visibility.sql`
2. `prod/20260826000500_create_game_records.sql`
3. `prod/20260826001000_game_record_ownership.sql`

Every `psql` invocation uses `ON_ERROR_STOP`; a failed file prevents later files
and contract verification from running. The SQL is idempotent and intentionally
does not add a separate migration-history system.

## Contract and isolation verification

Postflight temporarily grants the relevant runtime login to the administrative
session inside one transaction, verifies the same objects, columns and schema
permissions as application startup, resets the role, revokes the membership and
verifies cleanup before commit. Physical column reporting uses `pg_catalog` so
the administrator's `information_schema` visibility cannot produce a false
negative; the final assertion runs as the actual runtime role.

An optional deeper permission probe is available only by explicit request:

```sh
./migrate-db.sh prod --verify-isolation
./migrate-db.sh test --verify-isolation
```

This runs `admin/verify_environment_isolation.sql`. Its sentinel writes and
temporary role memberships are enclosed by the probe's final `ROLLBACK`, so it
does not leave production or test rows behind. The normal release does not run
this data-writing probe.

## Rollback

There is no automatic destructive DOWN migration. If an application revision
fails, roll Cloud Run back to the previous revision first. If the schema itself
must be reversed, use a Cloud SQL backup restore or a separately reviewed and
approved corrective migration. Do not run legacy `server/migrations` files or
the old schema-split rollback as an automatic response.
