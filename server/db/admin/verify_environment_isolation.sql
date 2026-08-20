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

DO $test_cannot_write_prod$
BEGIN
    BEGIN
        PERFORM count(*) FROM prod.custom_piece_versions;
        RAISE EXCEPTION 'test_app unexpectedly read prod schema';
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

DO $prod_cannot_write_test$
BEGIN
    BEGIN
        PERFORM count(*) FROM test.custom_piece_versions;
        RAISE EXCEPTION 'prod_app unexpectedly read test schema';
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
