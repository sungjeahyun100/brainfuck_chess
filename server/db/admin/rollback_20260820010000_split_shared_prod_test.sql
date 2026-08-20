-- Emergency reverse migration for the shared/prod/test schema split.
-- Run only with the old application revisions selected for rollback.
-- Test-environment tables and rows remain isolated in the test schema.
BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-schema-split-rollback-v1', 0));

DO $admin_guard$
BEGIN
    IF current_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION
          'run schema rollback as the Cloud SQL postgres administrator, not runtime role %',
          current_user;
    END IF;
END
$admin_guard$;

SET LOCAL ROLE deck_chess;

DO $rollback$
DECLARE
    public_count INTEGER;
    source_count INTEGER;
    users_before BIGINT;
    identities_before BIGINT;
    versions_before BIGINT;
    images_before BIGINT;
BEGIN
    SELECT count(*) INTO public_count
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE c.relkind = 'r'
      AND n.nspname = 'public'
      AND c.relname IN ('users', 'auth_identities', 'custom_piece_versions', 'custom_piece_images');

    SELECT count(*) INTO source_count
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE c.relkind = 'r'
      AND (n.nspname, c.relname) IN (
        ('shared', 'users'), ('shared', 'auth_identities'),
        ('prod', 'custom_piece_versions'), ('prod', 'custom_piece_images')
      );

    IF public_count = 0 AND source_count = 4 THEN
        SELECT count(*) INTO users_before FROM shared.users;
        SELECT count(*) INTO identities_before FROM shared.auth_identities;
        SELECT count(*) INTO versions_before FROM prod.custom_piece_versions;
        SELECT count(*) INTO images_before FROM prod.custom_piece_images;

        ALTER TABLE shared.users SET SCHEMA public;
        ALTER TABLE shared.auth_identities SET SCHEMA public;
        ALTER TABLE prod.custom_piece_versions SET SCHEMA public;
        ALTER TABLE prod.custom_piece_images SET SCHEMA public;

        IF users_before <> (SELECT count(*) FROM public.users)
           OR identities_before <> (SELECT count(*) FROM public.auth_identities)
           OR versions_before <> (SELECT count(*) FROM public.custom_piece_versions)
           OR images_before <> (SELECT count(*) FROM public.custom_piece_images) THEN
            RAISE EXCEPTION 'row-count verification failed; rolling back reverse migration';
        END IF;
    ELSIF NOT (public_count = 4 AND source_count = 0) THEN
        RAISE EXCEPTION
          'refusing partial schema rollback (public tables %, source tables %)',
          public_count, source_count;
    END IF;
END
$rollback$;

RESET ROLE;

-- Restore the legacy test application's public-table access. Production owns
-- the moved tables already. The test schema and its data remain untouched.
GRANT USAGE ON SCHEMA public TO deck_chess, deck_chess_test;
GRANT SELECT, INSERT, UPDATE, DELETE
    ON public.users, public.auth_identities,
       public.custom_piece_versions, public.custom_piece_images
    TO deck_chess_test;

COMMIT;
