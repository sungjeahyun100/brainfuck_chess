# Production data migrations

Production-only migrations for server-side decks, custom pieces and images belong
here. Run them with an administrative migration identity during an explicitly
approved production database release, never from a test deployment.

After the shared profile-visibility release, apply these production files in
order:

1. `20260826000500_create_game_records.sql` creates or validates the complete
   `prod.game_records` table and grants `prod_app` only SELECT/INSERT/UPDATE.
2. `20260826001000_game_record_ownership.sql` performs the lossless ownership
   backfill for an existing table and is a safe no-op for new empty tables.

The scripts never create or read `test.game_records`. Existing rows whose
current public ID cannot be matched remain nullable and private to third parties.
