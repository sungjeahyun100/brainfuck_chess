-- Production game-record release. Run as the approved database administrator.
BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-prod-create-game-records-v1', 0));

DO $admin_guard$
BEGIN
    IF session_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION 'run the prod game-record migration as the database administrator, not runtime role %', session_user;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'prod')
       OR NOT EXISTS (
           SELECT 1 FROM pg_class relation
           JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
           WHERE namespace.nspname = 'shared' AND relation.relname = 'users'
             AND relation.relkind IN ('r', 'p')
       ) THEN
        RAISE EXCEPTION 'shared/prod schema split must be applied before game records';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess_schema_owner')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'prod_app')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'test_app') THEN
        RAISE EXCEPTION 'schema-owner and application roles must exist before game records';
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname IN ('deck_chess', 'deck_chess_schema_owner')
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'administrator % must not retain migration owner memberships before migration', session_user;
    END IF;
    EXECUTE format('GRANT deck_chess TO %I', session_user);
    EXECUTE format('GRANT deck_chess_schema_owner TO %I', session_user);
END
$admin_guard$;

-- shared.users remains owned by the production runtime role after the schema
-- split, so only that owner context may grant the FK prerequisites.
SET LOCAL ROLE deck_chess;
GRANT SELECT, REFERENCES ON shared.users TO deck_chess_schema_owner;
RESET ROLE;

DO $normalize_existing_owner$
BEGIN
    IF to_regclass('prod.game_records') IS NOT NULL THEN
        ALTER TABLE prod.game_records OWNER TO deck_chess_schema_owner;
    END IF;
END
$normalize_existing_owner$;

SET LOCAL ROLE deck_chess_schema_owner;

CREATE TABLE IF NOT EXISTS prod.game_records (
    id TEXT PRIMARY KEY,
    white_public_id TEXT,
    black_public_id TEXT,
    white_user_id TEXT,
    black_user_id TEXT,
    started_at_ms BIGINT NOT NULL,
    ended_at_ms BIGINT,
    result_reason TEXT,
    display_name TEXT NOT NULL,
    record_version INTEGER NOT NULL,
    record JSONB NOT NULL,
    CONSTRAINT game_records_white_user_fk
        FOREIGN KEY (white_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT,
    CONSTRAINT game_records_black_user_fk
        FOREIGN KEY (black_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT
);

-- Existing tables must match the legacy record contract before ownership is added.
DO $base_contract$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM (VALUES
            ('id', 'text', 'NO'),
            ('white_public_id', 'text', 'YES'),
            ('black_public_id', 'text', 'YES'),
            ('started_at_ms', 'bigint', 'NO'),
            ('ended_at_ms', 'bigint', 'YES'),
            ('result_reason', 'text', 'YES'),
            ('display_name', 'text', 'NO'),
            ('record_version', 'integer', 'NO'),
            ('record', 'jsonb', 'NO')
        ) expected(column_name, data_type, is_nullable)
        WHERE NOT EXISTS (
            SELECT 1 FROM information_schema.columns actual
            WHERE actual.table_schema = 'prod'
              AND actual.table_name = 'game_records'
              AND actual.column_name = expected.column_name
              AND actual.data_type = expected.data_type
              AND actual.is_nullable = expected.is_nullable
        )
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prod.game_records'::regclass
          AND contype = 'p'
          AND pg_get_constraintdef(oid) = 'PRIMARY KEY (id)'
    ) THEN
        RAISE EXCEPTION 'existing prod.game_records does not match the required base contract';
    END IF;
END
$base_contract$;

ALTER TABLE prod.game_records
    ADD COLUMN IF NOT EXISTS white_user_id TEXT,
    ADD COLUMN IF NOT EXISTS black_user_id TEXT;

DO $ownership_constraints$
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
$ownership_constraints$;

CREATE INDEX IF NOT EXISTS game_records_white_user_started ON prod.game_records (white_user_id, started_at_ms DESC);
CREATE INDEX IF NOT EXISTS game_records_black_user_started ON prod.game_records (black_user_id, started_at_ms DESC);

REVOKE ALL ON prod.game_records FROM PUBLIC, prod_app, test_app, deck_chess, deck_chess_test;
GRANT SELECT, INSERT, UPDATE ON prod.game_records TO prod_app;

DO $verify$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM (VALUES ('white_user_id'), ('black_user_id')) expected(column_name)
        WHERE NOT EXISTS (
            SELECT 1 FROM information_schema.columns actual
            WHERE actual.table_schema = 'prod'
              AND actual.table_name = 'game_records'
              AND actual.column_name = expected.column_name
              AND actual.data_type = 'text'
              AND actual.is_nullable = 'YES'
        )
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'prod.game_records'::regclass
          AND contype = 'f' AND confrelid = 'shared.users'::regclass AND confdeltype = 'r'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (white_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'prod.game_records'::regclass
          AND contype = 'f' AND confrelid = 'shared.users'::regclass AND confdeltype = 'r'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (black_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE schemaname = 'prod' AND tablename = 'game_records'
          AND indexname = 'game_records_white_user_started' AND indexdef LIKE '%(white_user_id, started_at_ms DESC)%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE schemaname = 'prod' AND tablename = 'game_records'
          AND indexname = 'game_records_black_user_started' AND indexdef LIKE '%(black_user_id, started_at_ms DESC)%'
    ) OR NOT has_table_privilege('prod_app', 'prod.game_records', 'SELECT')
       OR NOT has_table_privilege('prod_app', 'prod.game_records', 'INSERT')
       OR NOT has_table_privilege('prod_app', 'prod.game_records', 'UPDATE')
       OR has_table_privilege('prod_app', 'prod.game_records', 'DELETE')
       OR has_table_privilege('test_app', 'prod.game_records', 'SELECT')
       OR has_schema_privilege('test_app', 'prod', 'USAGE') THEN
        RAISE EXCEPTION 'prod.game_records structure or privilege verification failed';
    END IF;
END
$verify$;

RESET ROLE;

DO $remove_admin_membership$
BEGIN
    EXECUTE format('REVOKE deck_chess_schema_owner FROM %I', session_user);
    EXECUTE format('REVOKE deck_chess FROM %I', session_user);
END
$remove_admin_membership$;

DO $verify_membership_cleanup$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname IN ('deck_chess', 'deck_chess_schema_owner')
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'temporary prod migration owner membership was not removed';
    END IF;
END
$verify_membership_cleanup$;

COMMIT;
