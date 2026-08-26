\set ON_ERROR_STOP on

DO $verify$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname IN ('deck_chess', 'deck_chess_schema_owner')
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'migration administrator retained a temporary owner membership';
    END IF;
    IF NOT has_schema_privilege('deck_chess', 'shared', 'USAGE')
       OR NOT has_schema_privilege('deck_chess', 'prod', 'USAGE')
       OR has_schema_privilege('deck_chess', 'test', 'USAGE')
       OR NOT has_schema_privilege('deck_chess_test', 'shared', 'USAGE')
       OR NOT has_schema_privilege('deck_chess_test', 'test', 'USAGE')
       OR has_schema_privilege('deck_chess_test', 'prod', 'USAGE') THEN
        RAISE EXCEPTION 'runtime schema isolation contract failed';
    END IF;
    IF NOT has_column_privilege('prod_app', 'shared.users', 'profile_visibility', 'UPDATE')
       OR NOT has_column_privilege('test_app', 'shared.users', 'profile_visibility', 'UPDATE') THEN
        RAISE EXCEPTION 'profile visibility column grants are missing';
    END IF;
    IF NOT has_table_privilege('prod_app', 'prod.game_records', 'SELECT,INSERT,UPDATE')
       OR has_table_privilege('prod_app', 'prod.game_records', 'DELETE')
       OR NOT has_table_privilege('test_app', 'test.game_records', 'SELECT,INSERT,UPDATE')
       OR has_table_privilege('test_app', 'test.game_records', 'DELETE') THEN
        RAISE EXCEPTION 'game record application grants are incorrect';
    END IF;
    IF (SELECT count(*) FROM pg_constraint
        WHERE conrelid = 'prod.game_records'::regclass AND contype = 'f'
          AND confrelid = 'shared.users'::regclass) <> 2
       OR (SELECT count(*) FROM pg_constraint
           WHERE conrelid = 'test.game_records'::regclass AND contype = 'f'
             AND confrelid = 'shared.users'::regclass) <> 2 THEN
        RAISE EXCEPTION 'game record FK constraints are missing or duplicated';
    END IF;
END
$verify$;

SET ROLE test_app;
INSERT INTO test.game_records
    (id, white_public_id, white_user_id, started_at_ms, display_name, record_version, record)
VALUES ('test-fixture', 'fixture-public', 'fixture-user', 1, 'test', 2, '{}');

DO $test_isolation$
BEGIN
    BEGIN
        PERFORM count(*) FROM prod.game_records;
        RAISE EXCEPTION 'test_app unexpectedly accessed prod.game_records';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        INSERT INTO test.game_records
            (id, white_user_id, started_at_ms, display_name, record_version, record)
        VALUES ('bad-test-fk', 'missing-user', 1, 'bad', 2, '{}');
        RAISE EXCEPTION 'test.game_records FK unexpectedly accepted a missing user';
    EXCEPTION WHEN foreign_key_violation THEN
        NULL;
    END;
END
$test_isolation$;
RESET ROLE;

SET ROLE prod_app;
INSERT INTO prod.game_records
    (id, black_public_id, black_user_id, started_at_ms, display_name, record_version, record)
VALUES ('prod-fixture', 'fixture-public', 'fixture-user', 1, 'prod', 2, '{}');

DO $prod_isolation$
BEGIN
    BEGIN
        PERFORM count(*) FROM test.game_records;
        RAISE EXCEPTION 'prod_app unexpectedly accessed test.game_records';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
END
$prod_isolation$;
RESET ROLE;
