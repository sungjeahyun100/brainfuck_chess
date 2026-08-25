BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-profile-visibility-v1', 0));

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

COMMIT;
