-- G9 disposable-only expanded ownership fixture; never use on an existing DB.
-- Uses postgres database, then the runner renames it to deck_chess.
\set ON_ERROR_STOP on

-- Disposable PostgreSQL-only fixture. Never run against a shared environment.
CREATE ROLE deck_chess LOGIN;
CREATE ROLE deck_chess_test LOGIN;
CREATE ROLE deck_chess_schema_owner NOLOGIN NOINHERIT;
CREATE ROLE prod_app NOLOGIN NOINHERIT;
CREATE ROLE test_app NOLOGIN NOINHERIT;

GRANT CREATE ON DATABASE postgres TO deck_chess_schema_owner;
SET ROLE deck_chess_schema_owner;
CREATE SCHEMA shared;
CREATE SCHEMA prod;
CREATE SCHEMA test;
RESET ROLE;

REVOKE CREATE ON DATABASE postgres FROM deck_chess_schema_owner;

SET ROLE deck_chess_schema_owner;
REVOKE ALL ON SCHEMA shared, prod, test FROM PUBLIC;
GRANT USAGE ON SCHEMA shared, prod TO prod_app;
GRANT USAGE ON SCHEMA shared, test TO test_app;
GRANT USAGE, CREATE ON SCHEMA shared TO deck_chess;
RESET ROLE;

GRANT prod_app TO deck_chess;
GRANT test_app TO deck_chess_test;
SET ROLE deck_chess;
CREATE TABLE shared.users (
    id TEXT PRIMARY KEY,
    account_kind TEXT NOT NULL CHECK (account_kind IN ('guest', 'registered')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    display_name TEXT,
    avatar_url TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE shared.auth_identities (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE RESTRICT,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    provider TEXT NOT NULL,
    email TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE (issuer, subject)
);

CREATE INDEX auth_identities_user_id ON shared.auth_identities (user_id);

ALTER TABLE shared.users ADD COLUMN public_id TEXT UNIQUE; ALTER TABLE shared.users ALTER COLUMN account_kind SET DEFAULT 'guest'; ALTER TABLE shared.users ALTER COLUMN created_at SET DEFAULT 0; ALTER TABLE shared.users ALTER COLUMN updated_at SET DEFAULT 0; INSERT INTO shared.users(id,public_id) VALUES ('fixture-user','fixture_public');
RESET ROLE;
SET ROLE deck_chess_schema_owner;
CREATE TABLE prod.custom_piece_versions (
    piece_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    owner_id TEXT NOT NULL,
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

CREATE INDEX custom_piece_versions_owner_latest
    ON prod.custom_piece_versions (owner_id, piece_id, version DESC);

CREATE TABLE prod.custom_piece_images (
    asset_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    media_type TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    bytes BYTEA NOT NULL
);

CREATE INDEX custom_piece_images_owner ON prod.custom_piece_images (owner_id);
CREATE TABLE test.custom_piece_versions (
    piece_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    owner_id TEXT NOT NULL,
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

CREATE INDEX custom_piece_versions_owner_latest
    ON test.custom_piece_versions (owner_id, piece_id, version DESC);

CREATE TABLE test.custom_piece_images (
    asset_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    media_type TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    bytes BYTEA NOT NULL
);

CREATE INDEX custom_piece_images_owner ON test.custom_piece_images (owner_id);
RESET ROLE;
GRANT SELECT, INSERT, UPDATE, DELETE ON shared.users, shared.auth_identities TO prod_app, test_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON prod.custom_piece_versions, prod.custom_piece_images TO prod_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON test.custom_piece_versions, test.custom_piece_images TO test_app;
REVOKE CREATE ON SCHEMA shared FROM deck_chess;
RESET ROLE;
