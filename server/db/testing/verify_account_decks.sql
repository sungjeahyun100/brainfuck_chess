\set ON_ERROR_STOP on
-- Run only against the disposable ownership fixture after current release SQL.
BEGIN;
INSERT INTO prod.decks (id,owner_id,name,deck_data,created_at_ms,updated_at_ms,request_key,create_hash)
VALUES ('11111111-1111-4111-8111-111111111111','fixture-user','production','{}',0,0,'fixture-prod','hash');
SET LOCAL ROLE test_app;
INSERT INTO test.decks (id,owner_id,name,deck_data,created_at_ms,updated_at_ms,request_key,create_hash)
VALUES ('11111111-1111-4111-8111-111111111111','fixture-user','test','{}',0,0,'fixture-test','hash');
UPDATE test.decks SET version=version+1, name='updated test'
WHERE id='11111111-1111-4111-8111-111111111111' AND owner_id='fixture-user' AND version=1;
DO $test$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM test.decks WHERE owner_id='fixture-user' AND name='updated test' AND version=2) THEN
        RAISE EXCEPTION 'test runtime CRUD failed';
    END IF;
    BEGIN
        PERFORM * FROM prod.decks;
        RAISE EXCEPTION 'test runtime can read prod decks';
    EXCEPTION WHEN insufficient_privilege THEN NULL;
    END;
    BEGIN
        INSERT INTO test.decks (id,owner_id,name,deck_data,created_at_ms,updated_at_ms,request_key,create_hash)
        VALUES ('22222222-2222-4222-8222-222222222222','missing-owner','invalid','{}',0,0,'invalid','hash');
        RAISE EXCEPTION 'deck owner FK not enforced';
    EXCEPTION WHEN foreign_key_violation THEN NULL;
    END;
END
$test$;
DELETE FROM test.decks WHERE id='11111111-1111-4111-8111-111111111111';
RESET ROLE;
SET LOCAL ROLE prod_app;
DO $test$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM prod.decks WHERE id='11111111-1111-4111-8111-111111111111' AND name='production' AND version=1) THEN
        RAISE EXCEPTION 'test operations affected prod deck';
    END IF;
    BEGIN
        PERFORM * FROM test.decks;
        RAISE EXCEPTION 'prod runtime can read test decks';
    EXCEPTION WHEN insufficient_privilege THEN NULL;
    END;
END
$test$;
RESET ROLE;
ROLLBACK;
SELECT 'account deck CRUD, FK and bidirectional environment isolation passed' AS result;
