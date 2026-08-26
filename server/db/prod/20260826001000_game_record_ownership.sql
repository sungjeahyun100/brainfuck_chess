BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-prod-game-record-ownership-v1', 0));

DO $admin_guard$
BEGIN
    IF session_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION 'run the prod ownership migration as the database administrator, not runtime role %', session_user;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess_schema_owner') THEN
        RAISE EXCEPTION 'schema-owner role must exist before the ownership migration';
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname = 'deck_chess_schema_owner'
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'administrator % must not retain schema-owner membership before migration', session_user;
    END IF;
    EXECUTE format('GRANT deck_chess_schema_owner TO %I', session_user);
END
$admin_guard$;

SET LOCAL ROLE deck_chess_schema_owner;

-- Resolve isolated objects only after assuming the schema owner. The Cloud SQL
-- administrator intentionally has no permanent prod schema USAGE.
DO $game_records_guard$
BEGIN
    IF to_regclass('prod.game_records') IS NULL THEN
        RAISE EXCEPTION 'apply 20260826000500_create_game_records.sql before the ownership migration';
    END IF;
END
$game_records_guard$;

ALTER TABLE prod.game_records
    ADD COLUMN IF NOT EXISTS white_user_id TEXT,
    ADD COLUMN IF NOT EXISTS black_user_id TEXT;

UPDATE prod.game_records records
SET white_user_id = users.id
FROM shared.users users
WHERE records.white_user_id IS NULL
  AND records.white_public_id = users.public_id;

UPDATE prod.game_records records
SET black_user_id = users.id
FROM shared.users users
WHERE records.black_user_id IS NULL
  AND records.black_public_id = users.public_id;

DO $constraints$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prod.game_records'::regclass
          AND contype = 'f'
          AND confrelid = 'shared.users'::regclass
          AND confdeltype = 'r'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (white_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT%'
    ) THEN
        ALTER TABLE prod.game_records ADD CONSTRAINT game_records_white_user_fk FOREIGN KEY (white_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prod.game_records'::regclass
          AND contype = 'f'
          AND confrelid = 'shared.users'::regclass
          AND confdeltype = 'r'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (black_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT%'
    ) THEN
        ALTER TABLE prod.game_records ADD CONSTRAINT game_records_black_user_fk FOREIGN KEY (black_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT;
    END IF;
END
$constraints$;

CREATE INDEX IF NOT EXISTS game_records_white_user_started ON prod.game_records (white_user_id, started_at_ms DESC);
CREATE INDEX IF NOT EXISTS game_records_black_user_started ON prod.game_records (black_user_id, started_at_ms DESC);

REVOKE ALL ON prod.game_records FROM PUBLIC, prod_app, test_app, deck_chess, deck_chess_test;
GRANT SELECT, INSERT, UPDATE ON prod.game_records TO prod_app;

DO $verify$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns WHERE table_schema = 'prod' AND table_name = 'game_records'
          AND column_name = 'white_user_id' AND data_type = 'text' AND is_nullable = 'YES'
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns WHERE table_schema = 'prod' AND table_name = 'game_records'
          AND column_name = 'black_user_id' AND data_type = 'text' AND is_nullable = 'YES'
    ) OR NOT has_table_privilege('prod_app', 'prod.game_records', 'SELECT')
       OR NOT has_table_privilege('prod_app', 'prod.game_records', 'INSERT')
       OR NOT has_table_privilege('prod_app', 'prod.game_records', 'UPDATE')
       OR has_table_privilege('prod_app', 'prod.game_records', 'DELETE')
       OR has_schema_privilege('test_app', 'prod', 'USAGE') THEN
        RAISE EXCEPTION 'prod game-record ownership verification failed';
    END IF;
END
$verify$;

RESET ROLE;

DO $remove_admin_membership$
BEGIN
    EXECUTE format('REVOKE deck_chess_schema_owner FROM %I', session_user);
END
$remove_admin_membership$;

DO $verify_membership_cleanup$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname = 'deck_chess_schema_owner'
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'temporary prod schema-owner administrator membership was not removed';
    END IF;
END
$verify_membership_cleanup$;

COMMIT;
