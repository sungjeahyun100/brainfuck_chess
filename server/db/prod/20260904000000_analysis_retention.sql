BEGIN;
SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-prod-analysis-retention-v1', 0));
DO $admin_guard$ BEGIN
 IF session_user IN ('deck_chess','deck_chess_test') THEN RAISE EXCEPTION 'run as database administrator, not runtime role %',session_user; END IF;
 IF EXISTS (SELECT 1 FROM pg_auth_members m JOIN pg_roles r ON r.oid=m.roleid JOIN pg_roles u ON u.oid=m.member WHERE r.rolname='deck_chess_schema_owner' AND u.rolname=session_user) THEN RAISE EXCEPTION 'administrator % must not retain schema-owner membership',session_user; END IF;
 EXECUTE format('GRANT deck_chess_schema_owner TO %I',session_user);
END $admin_guard$;
SET LOCAL ROLE deck_chess_schema_owner;

ALTER TABLE prod.game_records ADD COLUMN IF NOT EXISTS retention_mode TEXT;
ALTER TABLE prod.game_records ADD COLUMN IF NOT EXISTS expires_at_ms BIGINT;
UPDATE prod.game_records SET retention_mode='permanent' WHERE retention_mode IS NULL;
ALTER TABLE prod.game_records ALTER COLUMN retention_mode SET DEFAULT 'permanent', ALTER COLUMN retention_mode SET NOT NULL;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='prod.game_records'::regclass AND conname='game_records_retention_mode_check') THEN ALTER TABLE prod.game_records ADD CONSTRAINT game_records_retention_mode_check CHECK (retention_mode IN ('auto','permanent')); END IF; END $$;

CREATE TABLE IF NOT EXISTS prod.game_analysis_trees (
 id TEXT PRIMARY KEY, game_id TEXT NOT NULL REFERENCES prod.game_records(id) ON DELETE CASCADE,
 owner_user_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE RESTRICT,
 name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80), base_ply INTEGER NOT NULL CHECK(base_ply>=0),
 version BIGINT NOT NULL DEFAULT 1, request_id TEXT NOT NULL, created_at_ms BIGINT NOT NULL, updated_at_ms BIGINT NOT NULL,
 UNIQUE(owner_user_id,request_id)
);
CREATE TABLE IF NOT EXISTS prod.game_analysis_nodes (
 id TEXT PRIMARY KEY, analysis_tree_id TEXT NOT NULL REFERENCES prod.game_analysis_trees(id) ON DELETE CASCADE,
 parent_node_id TEXT, action JSONB NOT NULL, state_after JSONB NOT NULL, state_hash TEXT NOT NULL,
 request_id TEXT NOT NULL, created_at_ms BIGINT NOT NULL, UNIQUE(id,analysis_tree_id), UNIQUE(analysis_tree_id,request_id),
 FOREIGN KEY(parent_node_id,analysis_tree_id) REFERENCES prod.game_analysis_nodes(id,analysis_tree_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS game_records_expiry ON prod.game_records(expires_at_ms) WHERE retention_mode='auto';
CREATE INDEX IF NOT EXISTS game_analysis_trees_game_owner ON prod.game_analysis_trees(game_id,owner_user_id);
CREATE INDEX IF NOT EXISTS game_analysis_nodes_tree_parent ON prod.game_analysis_nodes(analysis_tree_id,parent_node_id);
REVOKE ALL ON prod.game_analysis_trees,prod.game_analysis_nodes FROM PUBLIC,test_app;
GRANT SELECT,INSERT,UPDATE,DELETE ON prod.game_analysis_trees,prod.game_analysis_nodes TO prod_app;
GRANT DELETE ON prod.game_records TO prod_app;
RESET ROLE;
DO $cleanup$ BEGIN EXECUTE format('REVOKE deck_chess_schema_owner FROM %I',session_user); END $cleanup$;
COMMIT;
