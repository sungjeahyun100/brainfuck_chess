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
    public_id TEXT UNIQUE,
    account_kind TEXT NOT NULL DEFAULT 'guest',
    status TEXT NOT NULL DEFAULT 'active',
    created_at BIGINT NOT NULL DEFAULT 0,
    updated_at BIGINT NOT NULL DEFAULT 0
);
CREATE TABLE shared.auth_identities (id TEXT PRIMARY KEY);
INSERT INTO shared.users (id, public_id) VALUES ('fixture-user', 'fixture-public');
RESET ROLE;

SET ROLE deck_chess_schema_owner;
CREATE TABLE prod.custom_piece_versions (id TEXT PRIMARY KEY);
CREATE TABLE prod.custom_piece_images (id TEXT PRIMARY KEY);
CREATE TABLE test.custom_piece_versions (id TEXT PRIMARY KEY);
CREATE TABLE test.custom_piece_images (id TEXT PRIMARY KEY);
REVOKE CREATE ON SCHEMA shared FROM deck_chess;
RESET ROLE;
