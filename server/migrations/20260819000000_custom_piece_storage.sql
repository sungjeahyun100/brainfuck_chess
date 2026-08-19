CREATE TABLE custom_piece_versions (
    piece_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    owner_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    score INTEGER NOT NULL,
    image_kind TEXT NOT NULL CHECK (image_kind IN ('built_in', 'uploaded')),
    image_value TEXT NOT NULL,
    raw_script TEXT NOT NULL,
    exposed_piece_key TEXT NOT NULL,
    internal_piece_keys JSONB NOT NULL,
    validation_status TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    package JSONB NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    active BOOLEAN NOT NULL,
    PRIMARY KEY (piece_id, version)
);

CREATE INDEX custom_piece_versions_owner_latest
    ON custom_piece_versions (owner_id, piece_id, version DESC);

CREATE TABLE custom_piece_images (
    asset_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    media_type TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    bytes BYTEA NOT NULL
);

CREATE INDEX custom_piece_images_owner ON custom_piece_images (owner_id);
