-- Test-only Account deck persistence. Never touches prod.
BEGIN;
SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-test-account-decks-v1', 0));

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
    ) OR NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'test') THEN
        RAISE EXCEPTION 'shared/test schema split must exist first';
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
CREATE TABLE IF NOT EXISTS test.decks (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    format_version INTEGER NOT NULL DEFAULT 1 CHECK (format_version = 1),
    deck_data JSONB NOT NULL CHECK (jsonb_typeof(deck_data) = 'object' AND octet_length(deck_data::text) <= 131072),
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    request_key TEXT NOT NULL,
    create_hash TEXT NOT NULL,
    UNIQUE (owner_id, request_key)
);
CREATE INDEX IF NOT EXISTS decks_owner_updated ON test.decks (owner_id, updated_at_ms DESC, id);
REVOKE ALL ON test.decks FROM PUBLIC, prod_app, test_app, deck_chess, deck_chess_test;
GRANT SELECT, INSERT, UPDATE, DELETE ON test.decks TO test_app;
DO $verify$
BEGIN
    IF EXISTS (
        SELECT 1 FROM (VALUES
            ('id','text'), ('owner_id','text'), ('name','text'), ('format_version','integer'),
            ('deck_data','jsonb'), ('created_at_ms','bigint'), ('updated_at_ms','bigint'),
            ('version','bigint'), ('request_key','text'), ('create_hash','text')
        ) expected(column_name, data_type)
        WHERE NOT EXISTS (SELECT 1 FROM information_schema.columns actual
            WHERE table_schema='test' AND table_name='decks' AND actual.column_name=expected.column_name
              AND actual.data_type=expected.data_type AND is_nullable='NO')
    ) OR NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='test.decks'::regclass
        AND contype='p' AND pg_get_constraintdef(oid)='PRIMARY KEY (id)')
      OR NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='test.decks'::regclass
        AND contype='u' AND pg_get_constraintdef(oid)='UNIQUE (owner_id, request_key)')
      OR NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='test.decks'::regclass
        AND contype='f' AND confrelid='shared.users'::regclass AND confdeltype='c'
        AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (owner_id) REFERENCES shared.users(id) ON DELETE CASCADE%')
      OR EXISTS (SELECT 1 FROM (VALUES ('SELECT'),('INSERT'),('UPDATE'),('DELETE')) p(privilege)
        WHERE NOT has_table_privilege('test_app', 'test.decks', p.privilege)
           OR has_table_privilege('prod_app', 'test.decks', p.privilege))
      OR (SELECT pg_get_userbyid(relowner) FROM pg_class WHERE oid='test.decks'::regclass) <> 'deck_chess_schema_owner'
    THEN RAISE EXCEPTION 'test.decks shape, owner or permission contract is invalid'; END IF;
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
