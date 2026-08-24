CREATE TABLE game_records (
    id TEXT PRIMARY KEY,
    white_public_id TEXT,
    black_public_id TEXT,
    started_at_ms BIGINT NOT NULL,
    ended_at_ms BIGINT,
    result_reason TEXT,
    display_name TEXT NOT NULL,
    record_version INTEGER NOT NULL,
    record JSONB NOT NULL
);

CREATE INDEX game_records_white_started ON game_records (white_public_id, started_at_ms DESC);
CREATE INDEX game_records_black_started ON game_records (black_public_id, started_at_ms DESC);
