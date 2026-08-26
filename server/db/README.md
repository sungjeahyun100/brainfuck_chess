# Administrative release SQL

Files under `shared`, `prod`, and `test` are ordered administrative release
scripts executed explicitly with `psql`; they are not registered in SQLx and do
not have migration-history checksums. Keep prod and test scripts symmetric where
their environment boundary is the only difference.

Use the repository-root `migrate-db.sh` runner for an ordered test or production
release. The exact operator workflow, connection requirements, backup step and
rollback policy are documented in `RELEASE.md`. The runner has explicit,
separate prod and test manifests; it never derives one environment's SQL from
the other environment's files.

The checksum-preserved immutable history is `../migrations`, as documented in
that directory. Do not apply those legacy files to a split-schema database.

The current Cloud SQL ownership contract intentionally gives the administrator
no permanent membership in `deck_chess` or `deck_chess_schema_owner`. Release
scripts may grant those roles to the administrative session only inside their
transaction, must `RESET ROLE`, and must revoke and verify the temporary
membership before commit. A failure rolls the membership changes back together
with the schema change.
