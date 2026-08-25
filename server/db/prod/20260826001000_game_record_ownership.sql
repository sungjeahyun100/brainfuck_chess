BEGIN;

SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-prod-game-record-ownership-v1', 0));

ALTER TABLE prod.game_records
    ADD COLUMN IF NOT EXISTS white_user_id TEXT,
    ADD COLUMN IF NOT EXISTS black_user_id TEXT;

UPDATE prod.game_records records
SET white_user_id = users.id
FROM shared.users users
WHERE records.white_user_id IS NULL
  AND records.white_public_id = users.public_id;

UPDATE prod.game_records records
SET black_user_id = users.id
FROM shared.users users
WHERE records.black_user_id IS NULL
  AND records.black_public_id = users.public_id;

DO $constraints$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'prod.game_records'::regclass AND conname = 'game_records_white_user_fk') THEN
        ALTER TABLE prod.game_records ADD CONSTRAINT game_records_white_user_fk FOREIGN KEY (white_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'prod.game_records'::regclass AND conname = 'game_records_black_user_fk') THEN
        ALTER TABLE prod.game_records ADD CONSTRAINT game_records_black_user_fk FOREIGN KEY (black_user_id) REFERENCES shared.users(id) ON DELETE RESTRICT;
    END IF;
END
$constraints$;

CREATE INDEX IF NOT EXISTS game_records_white_user_started ON prod.game_records (white_user_id, started_at_ms DESC);
CREATE INDEX IF NOT EXISTS game_records_black_user_started ON prod.game_records (black_user_id, started_at_ms DESC);

COMMIT;
