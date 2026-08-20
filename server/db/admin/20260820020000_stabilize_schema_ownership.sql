-- Cloud SQL built-in users inherit cloudsqlsuperuser, so that group must not
-- own isolated schemas. Keep schema/test-table ownership in a dedicated
-- NOLOGIN role that is never granted to either application login.
BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-schema-owner-v1', 0));

DO $admin_guard$
BEGIN
    IF current_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION
          'run schema ownership stabilization as the Cloud SQL administrator, not runtime role %',
          current_user;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess_schema_owner') THEN
        CREATE ROLE deck_chess_schema_owner
          NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
    END IF;
    EXECUTE format('GRANT deck_chess_schema_owner TO %I', current_user);
END
$admin_guard$;

DO $temporary_database_create$
BEGIN
    EXECUTE format(
      'GRANT CREATE ON DATABASE %I TO deck_chess_schema_owner',
      current_database()
    );
END
$temporary_database_create$;

ALTER SCHEMA shared OWNER TO deck_chess_schema_owner;
ALTER SCHEMA prod OWNER TO deck_chess_schema_owner;
ALTER SCHEMA test OWNER TO deck_chess_schema_owner;
ALTER TABLE test.custom_piece_versions OWNER TO deck_chess_schema_owner;
ALTER TABLE test.custom_piece_images OWNER TO deck_chess_schema_owner;

DO $remove_database_create$
BEGIN
    EXECUTE format(
      'REVOKE CREATE ON DATABASE %I FROM deck_chess_schema_owner',
      current_database()
    );
END
$remove_database_create$;

-- ALTER OWNER preserves existing object ACLs. Reassert the intended schema
-- boundary explicitly before removing the administrator's temporary membership.
REVOKE ALL ON SCHEMA shared, prod, test FROM PUBLIC, deck_chess, deck_chess_test;
GRANT USAGE ON SCHEMA shared, prod TO prod_app;
GRANT USAGE ON SCHEMA shared, test TO test_app;

DO $verify$
BEGIN
    IF has_schema_privilege('deck_chess', 'test', 'USAGE')
       OR has_schema_privilege('deck_chess_test', 'prod', 'USAGE')
       OR NOT has_schema_privilege('deck_chess', 'shared', 'USAGE')
       OR NOT has_schema_privilege('deck_chess', 'prod', 'USAGE')
       OR NOT has_schema_privilege('deck_chess_test', 'shared', 'USAGE')
       OR NOT has_schema_privilege('deck_chess_test', 'test', 'USAGE') THEN
        RAISE EXCEPTION 'schema ownership stabilization failed isolation contract';
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_auth_members m
        JOIN pg_roles parent ON parent.oid = m.roleid
        JOIN pg_roles member ON member.oid = m.member
        WHERE parent.rolname = 'deck_chess_schema_owner'
          AND member.rolname IN ('deck_chess', 'deck_chess_test')
    ) THEN
        RAISE EXCEPTION 'runtime login unexpectedly belongs to schema-owner role';
    END IF;
END
$verify$;

DO $remove_admin_membership$
BEGIN
    EXECUTE format('REVOKE deck_chess_schema_owner FROM %I', current_user);
END
$remove_admin_membership$;

COMMIT;
