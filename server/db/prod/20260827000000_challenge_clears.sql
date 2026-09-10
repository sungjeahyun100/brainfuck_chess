-- Production-only Challenge clear persistence. Apply with the approved admin identity.
BEGIN;
SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-prod-challenge-clears-v1', 0));

DO $guard$
BEGIN
    IF session_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION 'run this migration as an administrator, not runtime role %', session_user;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_class relation
        JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
        WHERE namespace.nspname = 'shared' AND relation.relname = 'users'
          AND relation.relkind IN ('r', 'p')
    ) OR NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'prod') THEN
        RAISE EXCEPTION 'shared/prod schema split must exist first';
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
$guard$;

SET LOCAL ROLE deck_chess_schema_owner;
CREATE TABLE IF NOT EXISTS prod.challenge_clears (
    user_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE CASCADE,
    challenge_id TEXT NOT NULL,
    first_cleared_at_ms BIGINT NOT NULL,
    PRIMARY KEY (user_id, challenge_id)
);
CREATE INDEX IF NOT EXISTS challenge_clears_user_first_cleared
    ON prod.challenge_clears (user_id, first_cleared_at_ms DESC);
REVOKE ALL ON prod.challenge_clears FROM PUBLIC, prod_app, test_app, deck_chess, deck_chess_test;
GRANT SELECT, INSERT ON prod.challenge_clears TO prod_app;

DO $verify$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM (VALUES
            ('user_id', 'text', 'NO'),
            ('challenge_id', 'text', 'NO'),
            ('first_cleared_at_ms', 'bigint', 'NO')
        ) expected(column_name, data_type, is_nullable)
        WHERE NOT EXISTS (
            SELECT 1 FROM information_schema.columns actual
            WHERE actual.table_schema = 'prod'
              AND actual.table_name = 'challenge_clears'
              AND actual.column_name = expected.column_name
              AND actual.data_type = expected.data_type
              AND actual.is_nullable = expected.is_nullable
        )
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prod.challenge_clears'::regclass AND contype = 'p'
          AND pg_get_constraintdef(oid) = 'PRIMARY KEY (user_id, challenge_id)'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'prod.challenge_clears'::regclass AND contype = 'f'
          AND confrelid = 'shared.users'::regclass AND confdeltype = 'c'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (user_id) REFERENCES shared.users(id) ON DELETE CASCADE%'
    ) OR NOT has_table_privilege('prod_app', 'prod.challenge_clears', 'SELECT')
       OR NOT has_table_privilege('prod_app', 'prod.challenge_clears', 'INSERT')
       OR has_table_privilege('prod_app', 'prod.challenge_clears', 'DELETE')
       OR has_table_privilege('test_app', 'prod.challenge_clears', 'SELECT') THEN
        RAISE EXCEPTION 'prod.challenge_clears privilege verification failed';
    END IF;
END
$verify$;
RESET ROLE;

DO $cleanup$
BEGIN
    EXECUTE format('REVOKE deck_chess_schema_owner FROM %I', session_user);
END
$cleanup$;
DO $cleanup_verify$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname = 'deck_chess_schema_owner'
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'temporary schema-owner membership was not removed';
    END IF;
END
$cleanup_verify$;
COMMIT;
