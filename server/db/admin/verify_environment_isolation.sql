-- Run as the database owner after the split migration. Every unexpected
-- permission raises and rolls the transaction back.
BEGIN;

INSERT INTO shared.users (id, account_kind, status, created_at, updated_at)
VALUES ('__deck_chess_permission_probe__', 'guest', 'active', 0, 0)
ON CONFLICT (id) DO NOTHING;

SET LOCAL ROLE test_app;
INSERT INTO test.custom_piece_images
    (asset_id, owner_id, media_type, width, height, content_hash, bytes)
VALUES ('__test_probe__', '__deck_chess_permission_probe__', 'image/png', 1, 1, 'probe', '\x00');

INSERT INTO test.game_records
    (id, white_public_id, black_public_id, white_user_id, black_user_id,
     started_at_ms, ended_at_ms, result_reason, display_name, record_version, record)
VALUES
    ('__test_game_record_probe__', 'test-white', 'test-black',
     '__deck_chess_permission_probe__', NULL, 0, NULL, NULL, 'test-probe', 1, '{}')
ON CONFLICT (id) DO UPDATE SET ended_at_ms = EXCLUDED.ended_at_ms;
SELECT record FROM test.game_records WHERE id = '__test_game_record_probe__';

INSERT INTO test.challenge_clears (user_id, challenge_id, first_cleared_at_ms)
VALUES ('__deck_chess_permission_probe__', '__test_challenge_probe__', 0)
ON CONFLICT (user_id, challenge_id) DO NOTHING;
SELECT challenge_id FROM test.challenge_clears
WHERE user_id = '__deck_chess_permission_probe__';

DO $test_cannot_write_prod$
BEGIN
    BEGIN
        PERFORM count(*) FROM prod.custom_piece_versions;
        RAISE EXCEPTION 'test_app unexpectedly read prod schema';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        PERFORM count(*) FROM prod.game_records;
        RAISE EXCEPTION 'test_app unexpectedly read prod game records';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        PERFORM count(*) FROM prod.challenge_clears;
        RAISE EXCEPTION 'test_app unexpectedly read prod challenge clears';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        INSERT INTO prod.custom_piece_images
            (asset_id, owner_id, media_type, width, height, content_hash, bytes)
        VALUES ('__forbidden_test_probe__', '__deck_chess_permission_probe__',
                'image/png', 1, 1, 'probe', '\x00');
        RAISE EXCEPTION 'test_app unexpectedly inserted into prod schema';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        DELETE FROM prod.custom_piece_images WHERE asset_id = '__never__';
        RAISE EXCEPTION 'test_app unexpectedly wrote prod schema';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
END
$test_cannot_write_prod$;

RESET ROLE;
SET LOCAL ROLE prod_app;
SELECT count(*) FROM shared.users;
INSERT INTO prod.custom_piece_images
    (asset_id, owner_id, media_type, width, height, content_hash, bytes)
VALUES ('__prod_probe__', '__deck_chess_permission_probe__', 'image/png', 1, 1, 'probe', '\x00');

INSERT INTO prod.game_records
    (id, white_public_id, black_public_id, white_user_id, black_user_id,
     started_at_ms, ended_at_ms, result_reason, display_name, record_version, record)
VALUES
    ('__prod_game_record_probe__', 'prod-white', 'prod-black',
     '__deck_chess_permission_probe__', NULL, 0, NULL, NULL, 'prod-probe', 1, '{}')
ON CONFLICT (id) DO UPDATE SET ended_at_ms = EXCLUDED.ended_at_ms;
SELECT record FROM prod.game_records WHERE id = '__prod_game_record_probe__';

INSERT INTO prod.challenge_clears (user_id, challenge_id, first_cleared_at_ms)
VALUES ('__deck_chess_permission_probe__', '__prod_challenge_probe__', 0)
ON CONFLICT (user_id, challenge_id) DO NOTHING;
SELECT challenge_id FROM prod.challenge_clears
WHERE user_id = '__deck_chess_permission_probe__';

DO $prod_cannot_write_test$
BEGIN
    BEGIN
        PERFORM count(*) FROM test.custom_piece_versions;
        RAISE EXCEPTION 'prod_app unexpectedly read test schema';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        PERFORM count(*) FROM test.game_records;
        RAISE EXCEPTION 'prod_app unexpectedly read test game records';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        PERFORM count(*) FROM test.challenge_clears;
        RAISE EXCEPTION 'prod_app unexpectedly read test challenge clears';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        INSERT INTO test.custom_piece_images
            (asset_id, owner_id, media_type, width, height, content_hash, bytes)
        VALUES ('__forbidden_prod_probe__', '__deck_chess_permission_probe__',
                'image/png', 1, 1, 'probe', '\x00');
        RAISE EXCEPTION 'prod_app unexpectedly inserted into test schema';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
    BEGIN
        DELETE FROM test.custom_piece_images WHERE asset_id = '__test_probe__';
        RAISE EXCEPTION 'prod_app unexpectedly wrote test schema';
    EXCEPTION WHEN insufficient_privilege THEN
        NULL;
    END;
END
$prod_cannot_write_test$;

RESET ROLE;
ROLLBACK;
