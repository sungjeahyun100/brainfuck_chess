BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-test-game-record-ownership-v1', 0));

DO $admin_guard$
BEGIN
    IF current_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION 'run the test ownership migration as the database administrator, not runtime role %', current_user;
    END IF;
    IF to_regclass('test.game_records') IS NULL THEN
        RAISE EXCEPTION 'apply 20260826000500_create_game_records.sql before the ownership migration';
    END IF;
    EXECUTE format('GRANT deck_chess_schema_owner TO %I', current_user);
END
$admin_guard$;

SET LOCAL ROLE deck_chess_schema_owner;

ALTER TABLE test.game_records
    ADD COLUMN IF NOT EXISTS white_user_id TEXT,
    ADD COLUMN IF NOT EXISTS black_user_id TEXT;

UPDATE test.game_records records
SET white_user_id = users.id
FROM shared.users users
WHERE records.white_user_id IS NULL
  AND records.white_public_id = users.public_id;

UPDATE test.game_records records
SET black_user_id = users.id
FROM shared.users users
WHERE records.black_user_id IS NULL
  AND records.black_public_id = users.public_id;

DO $constraints$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'test.game_records'::regclass
          AND contype = 'f'
          AND confrelid = 'shared.users'::regclass
          AND confdeltype = 'r'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (white_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT%'
    ) THEN
        ALTER TABLE test.game_records ADD CONSTRAINT game_records_white_user_fk FOREIGN KEY (white_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'test.game_records'::regclass
          AND contype = 'f'
          AND confrelid = 'shared.users'::regclass
          AND confdeltype = 'r'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (black_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT%'
    ) THEN
        ALTER TABLE test.game_records ADD CONSTRAINT game_records_black_user_fk FOREIGN KEY (black_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT;
    END IF;
END
$constraints$;

CREATE INDEX IF NOT EXISTS game_records_white_user_started ON test.game_records (white_user_id, started_at_ms DESC);
CREATE INDEX IF NOT EXISTS game_records_black_user_started ON test.game_records (black_user_id, started_at_ms DESC);

REVOKE ALL ON test.game_records FROM PUBLIC, prod_app, test_app, deck_chess, deck_chess_test;
GRANT SELECT, INSERT, UPDATE ON test.game_records TO test_app;

DO $verify$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns WHERE table_schema = 'test' AND table_name = 'game_records'
          AND column_name = 'white_user_id' AND data_type = 'text' AND is_nullable = 'YES'
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns WHERE table_schema = 'test' AND table_name = 'game_records'
          AND column_name = 'black_user_id' AND data_type = 'text' AND is_nullable = 'YES'
    ) OR NOT has_table_privilege('test_app', 'test.game_records', 'SELECT')
       OR NOT has_table_privilege('test_app', 'test.game_records', 'INSERT')
       OR NOT has_table_privilege('test_app', 'test.game_records', 'UPDATE')
       OR has_table_privilege('test_app', 'test.game_records', 'DELETE')
       OR has_schema_privilege('prod_app', 'test', 'USAGE') THEN
        RAISE EXCEPTION 'test game-record ownership verification failed';
    END IF;
END
$verify$;

RESET ROLE;

DO $remove_admin_membership$
BEGIN
    EXECUTE format('REVOKE deck_chess_schema_owner FROM %I', current_user);
END
$remove_admin_membership$;

COMMIT;
