CREATE TABLE users (
    id TEXT PRIMARY KEY,
    account_kind TEXT NOT NULL CHECK (account_kind IN ('guest', 'registered')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    display_name TEXT,
    avatar_url TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE auth_identities (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    provider TEXT NOT NULL,
    email TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE (issuer, subject)
);

CREATE INDEX auth_identities_user_id ON auth_identities (user_id);

INSERT INTO users (id, account_kind, status, created_at, updated_at)
SELECT owner_id, 'guest', 'active', EXTRACT(EPOCH FROM NOW())::BIGINT, EXTRACT(EPOCH FROM NOW())::BIGINT
FROM (
    SELECT DISTINCT owner_id FROM custom_piece_versions
    UNION
    SELECT DISTINCT owner_id FROM custom_piece_images
) existing_owners
ON CONFLICT (id) DO NOTHING;

ALTER TABLE custom_piece_versions
    ADD CONSTRAINT custom_piece_versions_owner_fk
    FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE RESTRICT;

ALTER TABLE custom_piece_images
    ADD CONSTRAINT custom_piece_images_owner_fk
    FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE RESTRICT;
