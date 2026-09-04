-- Development migration. Production/test releases use the schema-qualified
-- administrator scripts under server/db/{prod,test}.
ALTER TABLE game_records ADD COLUMN retention_mode TEXT NOT NULL DEFAULT 'permanent'
    CHECK (retention_mode IN ('auto', 'permanent'));
ALTER TABLE game_records ADD COLUMN expires_at_ms BIGINT;

CREATE TABLE game_analysis_trees (
    id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL REFERENCES game_records(id) ON DELETE CASCADE,
    owner_user_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    base_ply INTEGER NOT NULL CHECK (base_ply >= 0),
    version BIGINT NOT NULL DEFAULT 1,
    request_id TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    UNIQUE (owner_user_id, request_id)
);
CREATE TABLE game_analysis_nodes (
    id TEXT PRIMARY KEY,
    analysis_tree_id TEXT NOT NULL REFERENCES game_analysis_trees(id) ON DELETE CASCADE,
    parent_node_id TEXT,
    action JSONB NOT NULL,
    state_after JSONB NOT NULL,
    state_hash TEXT NOT NULL,
    request_id TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    UNIQUE (id, analysis_tree_id),
    UNIQUE (analysis_tree_id, request_id),
    FOREIGN KEY (parent_node_id, analysis_tree_id)
        REFERENCES game_analysis_nodes(id, analysis_tree_id) ON DELETE CASCADE
);
