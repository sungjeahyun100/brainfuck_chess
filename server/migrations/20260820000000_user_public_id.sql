ALTER TABLE users
    ADD COLUMN public_id TEXT;

ALTER TABLE users
    ADD CONSTRAINT users_public_id_format
    CHECK (
        public_id IS NULL
        OR public_id ~ '^[a-z0-9][a-z0-9_]{2,19}$'
    );

CREATE UNIQUE INDEX users_public_id_unique
    ON users (public_id)
    WHERE public_id IS NOT NULL;
