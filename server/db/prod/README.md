# Production data migrations

Production-only migrations for server-side decks, custom pieces and images belong
here. Run them with an administrative migration identity during an explicitly
approved production database release, never from a test deployment.

After the shared profile-visibility release, apply
`20260826001000_game_record_ownership.sql` to add internal participant ownership
to production records. It backfills uniquely matching current public IDs and
leaves guest or ambiguous historical ownership unset.
