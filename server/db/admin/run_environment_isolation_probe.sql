\set ON_ERROR_STOP on

-- The included probe writes sentinel rows only inside this transaction and
-- ends with ROLLBACK. Temporary memberships are rolled back with those rows.
BEGIN;

DO $guard$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname IN ('deck_chess', 'prod_app', 'test_app')
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'administrator already has a role required by the isolation probe';
    END IF;
    EXECUTE format('GRANT deck_chess, prod_app, test_app TO %I', session_user);
END
$guard$;

SET LOCAL ROLE deck_chess;
\ir verify_environment_isolation.sql

DO $verify_rollback_cleanup$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname IN ('deck_chess', 'prod_app', 'test_app')
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'isolation probe temporary membership survived rollback';
    END IF;
END
$verify_rollback_cleanup$;

\echo 'Environment isolation probe PASS; all probe writes and memberships rolled back'
