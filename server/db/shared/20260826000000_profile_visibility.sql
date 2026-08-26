BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-profile-visibility-v1', 0));

DO $admin_guard$
BEGIN
    IF session_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION 'run the profile visibility migration as the database administrator, not runtime role %', session_user;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'prod_app')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'test_app') THEN
        RAISE EXCEPTION 'table-owner and application roles must exist before profile visibility';
    END IF;
    IF NOT EXISTS (
        SELECT 1
        FROM pg_class relation
        JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
        JOIN pg_roles owner_role ON owner_role.oid = relation.relowner
        WHERE namespace.nspname = 'shared'
          AND relation.relname = 'users'
          AND relation.relkind IN ('r', 'p')
          AND owner_role.rolname = 'deck_chess'
    ) THEN
        RAISE EXCEPTION 'shared.users must exist and be owned by deck_chess before profile visibility';
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname = 'deck_chess' AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'administrator % must not retain deck_chess membership before migration', session_user;
    END IF;

    -- Cloud SQL administrators do not bypass table ownership. Membership is
    -- transactional and is removed again before commit.
    EXECUTE format('GRANT deck_chess TO %I', session_user);
END
$admin_guard$;

SET LOCAL ROLE deck_chess;

ALTER TABLE shared.users
    ADD COLUMN IF NOT EXISTS profile_visibility TEXT NOT NULL DEFAULT 'public';

DO $constraint$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'shared.users'::regclass
          AND conname = 'users_profile_visibility_check'
    ) THEN
        ALTER TABLE shared.users
            ADD CONSTRAINT users_profile_visibility_check
            CHECK (profile_visibility IN ('public', 'private'));
    END IF;
END
$constraint$;

GRANT UPDATE (profile_visibility) ON shared.users TO prod_app, test_app;

DO $verify$
BEGIN
    IF EXISTS (
        SELECT 1 FROM shared.users
        WHERE profile_visibility NOT IN ('public', 'private')
    ) OR NOT has_column_privilege('prod_app', 'shared.users', 'profile_visibility', 'UPDATE')
       OR NOT has_column_privilege('test_app', 'shared.users', 'profile_visibility', 'UPDATE') THEN
        RAISE EXCEPTION 'profile visibility migration verification failed';
    END IF;
END
$verify$;

RESET ROLE;

DO $remove_admin_membership$
BEGIN
    EXECUTE format('REVOKE deck_chess FROM %I', session_user);
END
$remove_admin_membership$;

DO $verify_membership_cleanup$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname = 'deck_chess' AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'temporary deck_chess administrator membership was not removed';
    END IF;
END
$verify_membership_cleanup$;

COMMIT;
