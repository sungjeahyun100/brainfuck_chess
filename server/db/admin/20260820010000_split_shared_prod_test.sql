-- Administrative, one-time migration. Run with the existing table owner after
-- taking a Cloud SQL backup. The application runtime never executes this file.
-- The whole move is transactional: a failed assertion restores the public layout.
BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-schema-split-v1', 0));

DO $admin_guard$
BEGIN
    IF current_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION
          'run schema split as the Cloud SQL postgres administrator, not runtime role %',
          current_user;
    END IF;
END
$admin_guard$;

CREATE SCHEMA IF NOT EXISTS shared;
CREATE SCHEMA IF NOT EXISTS prod;
CREATE SCHEMA IF NOT EXISTS test;

REVOKE ALL ON SCHEMA shared, prod, test FROM PUBLIC;

-- Existing application tables are owned by deck_chess. The administrative
-- session must be a member of that role, but schemas/test tables remain owned
-- by the administrator rather than the production runtime role.
GRANT USAGE, CREATE ON SCHEMA shared, prod TO deck_chess;
SET LOCAL ROLE deck_chess;

DO $migration$
DECLARE
    public_count INTEGER;
    target_count INTEGER;
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

    SELECT count(*) INTO target_count
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE c.relkind = 'r'
      AND (n.nspname, c.relname) IN (
        ('shared', 'users'), ('shared', 'auth_identities'),
        ('prod', 'custom_piece_versions'), ('prod', 'custom_piece_images')
      );

    IF public_count = 4 AND target_count = 0 THEN
        SELECT count(*) INTO users_before FROM public.users;
        SELECT count(*) INTO identities_before FROM public.auth_identities;
        SELECT count(*) INTO versions_before FROM public.custom_piece_versions;
        SELECT count(*) INTO images_before FROM public.custom_piece_images;

        -- SET SCHEMA preserves table OIDs, rows, indexes, PKs and FK references.
        ALTER TABLE public.users SET SCHEMA shared;
        ALTER TABLE public.auth_identities SET SCHEMA shared;
        ALTER TABLE public.custom_piece_versions SET SCHEMA prod;
        ALTER TABLE public.custom_piece_images SET SCHEMA prod;

        IF users_before <> (SELECT count(*) FROM shared.users)
           OR identities_before <> (SELECT count(*) FROM shared.auth_identities)
           OR versions_before <> (SELECT count(*) FROM prod.custom_piece_versions)
           OR images_before <> (SELECT count(*) FROM prod.custom_piece_images) THEN
            RAISE EXCEPTION 'row-count verification failed; rolling back schema split';
        END IF;
    ELSIF NOT (public_count = 0 AND target_count = 4) THEN
        RAISE EXCEPTION
          'refusing partial schema migration (public tables %, target tables %)',
          public_count, target_count;
    END IF;
END
$migration$;

RESET ROLE;

-- Test starts empty. It deliberately does not copy production rows.
CREATE TABLE IF NOT EXISTS test.custom_piece_versions (
    piece_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    owner_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    score INTEGER NOT NULL,
    image_kind TEXT NOT NULL CHECK (image_kind IN ('built_in', 'uploaded')),
    image_value TEXT NOT NULL,
    raw_script TEXT NOT NULL,
    exposed_piece_key TEXT NOT NULL,
    internal_piece_keys JSONB NOT NULL,
    validation_status TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    package JSONB NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    active BOOLEAN NOT NULL,
    PRIMARY KEY (piece_id, version)
);

CREATE INDEX IF NOT EXISTS custom_piece_versions_owner_latest
    ON test.custom_piece_versions (owner_id, piece_id, version DESC);

CREATE TABLE IF NOT EXISTS test.custom_piece_images (
    asset_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE RESTRICT,
    media_type TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    bytes BYTEA NOT NULL
);

CREATE INDEX IF NOT EXISTS custom_piece_images_owner
    ON test.custom_piece_images (owner_id);

-- Group roles contain no password. Grant exactly one of them to each existing
-- Cloud SQL login role after this migration (see docs/database-environment-isolation.md).
DO $roles$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'prod_app') THEN
        CREATE ROLE prod_app NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'test_app') THEN
        CREATE ROLE test_app NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
    END IF;
END
$roles$;

-- Remove both inherited and any legacy direct grants before rebuilding access.
REVOKE ALL ON SCHEMA shared, prod, test
    FROM prod_app, test_app, deck_chess, deck_chess_test;
REVOKE ALL ON ALL TABLES IN SCHEMA shared, prod, test
    FROM prod_app, test_app, deck_chess, deck_chess_test;

GRANT USAGE ON SCHEMA shared, prod TO prod_app;
GRANT SELECT, INSERT ON shared.users, shared.auth_identities TO prod_app;
GRANT UPDATE (account_kind, status, public_id, display_name, avatar_url, updated_at)
    ON shared.users TO prod_app;
GRANT UPDATE (email, email_verified, updated_at) ON shared.auth_identities TO prod_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA prod TO prod_app;

GRANT USAGE ON SCHEMA shared, test TO test_app;
GRANT SELECT, INSERT ON shared.users, shared.auth_identities TO test_app;
-- Test may update profile and the minimum fields required by the shared Google
-- login flow. It receives no DELETE and no blanket privilege on future shared tables.
GRANT UPDATE (account_kind, public_id, display_name, avatar_url, updated_at)
    ON shared.users TO test_app;
GRANT UPDATE (email, email_verified, updated_at) ON shared.auth_identities TO test_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA test TO test_app;

-- These existing Cloud SQL login roles contain the passwords referenced by the
-- two DATABASE_URL Secrets. Membership adds no new resource or credential.
GRANT prod_app TO deck_chess;
GRANT test_app TO deck_chess_test;

COMMIT;
