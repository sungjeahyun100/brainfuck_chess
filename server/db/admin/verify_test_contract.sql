\set ON_ERROR_STOP on

BEGIN;

DO $grant_runtime_role$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname = 'deck_chess_test'
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'administrator % already has deck_chess_test membership', session_user;
    END IF;
    EXECUTE format('GRANT deck_chess_test TO %I', session_user);
END
$grant_runtime_role$;

SET LOCAL ROLE deck_chess_test;

\echo 'Test database contract'
SELECT check_name,
       CASE WHEN passed THEN 'OK' ELSE 'FAIL' END AS status
FROM (VALUES
    ('shared.users', to_regclass('shared.users') IS NOT NULL),
    ('shared.auth_identities', to_regclass('shared.auth_identities') IS NOT NULL),
    ('test.custom_piece_versions', to_regclass('test.custom_piece_versions') IS NOT NULL),
    ('test.custom_piece_images', to_regclass('test.custom_piece_images') IS NOT NULL),
    ('test.game_records', to_regclass('test.game_records') IS NOT NULL),
    ('profile_visibility', EXISTS (
        SELECT 1 FROM pg_attribute attribute
        JOIN pg_class relation ON relation.oid = attribute.attrelid
        JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
        WHERE namespace.nspname = 'shared' AND relation.relname = 'users'
          AND attribute.attname = 'profile_visibility'
          AND attribute.attnum > 0 AND NOT attribute.attisdropped
    )),
    ('white_user_id', EXISTS (
        SELECT 1 FROM pg_attribute attribute
        JOIN pg_class relation ON relation.oid = attribute.attrelid
        JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
        WHERE namespace.nspname = 'test' AND relation.relname = 'game_records'
          AND attribute.attname = 'white_user_id'
          AND attribute.attnum > 0 AND NOT attribute.attisdropped
    )),
    ('black_user_id', EXISTS (
        SELECT 1 FROM pg_attribute attribute
        JOIN pg_class relation ON relation.oid = attribute.attrelid
        JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
        WHERE namespace.nspname = 'test' AND relation.relname = 'game_records'
          AND attribute.attname = 'black_user_id'
          AND attribute.attnum > 0 AND NOT attribute.attisdropped
    )),
    ('shared usage', has_schema_privilege(current_user, 'shared', 'USAGE')),
    ('test usage', has_schema_privilege(current_user, 'test', 'USAGE')),
    ('prod isolation', NOT has_schema_privilege(current_user, 'prod', 'USAGE'))
) checks(check_name, passed);

DO $verify_runtime_contract$
BEGIN
    IF to_regclass('shared.users') IS NULL
       OR to_regclass('shared.auth_identities') IS NULL
       OR to_regclass('test.custom_piece_versions') IS NULL
       OR to_regclass('test.custom_piece_images') IS NULL
       OR to_regclass('test.game_records') IS NULL
       OR NOT EXISTS (
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = 'shared' AND table_name = 'users'
             AND column_name = 'profile_visibility'
       )
       OR NOT EXISTS (
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = 'test' AND table_name = 'game_records'
             AND column_name = 'white_user_id'
       )
       OR NOT EXISTS (
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = 'test' AND table_name = 'game_records'
             AND column_name = 'black_user_id'
       )
       OR NOT has_schema_privilege(current_user, 'shared', 'USAGE')
       OR NOT has_schema_privilege(current_user, 'test', 'USAGE')
       OR has_schema_privilege(current_user, 'prod', 'USAGE') THEN
        RAISE EXCEPTION 'test runtime database contract failed';
    END IF;
END
$verify_runtime_contract$;

RESET ROLE;

DO $remove_runtime_role$
BEGIN
    EXECUTE format('REVOKE deck_chess_test FROM %I', session_user);
END
$remove_runtime_role$;

DO $verify_cleanup$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname IN ('deck_chess', 'deck_chess_test', 'deck_chess_schema_owner')
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'temporary test verification membership was not removed';
    END IF;
END
$verify_cleanup$;

COMMIT;
\echo 'Test database contract PASS; temporary role membership cleanup OK'
