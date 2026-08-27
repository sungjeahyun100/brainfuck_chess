-- Production-only Challenge clear persistence. Apply with the approved admin identity.
BEGIN;
SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-prod-challenge-clears-v1', 0));

DO $guard$
BEGIN
    IF session_user IN ('deck_chess', 'deck_chess_test') THEN
        RAISE EXCEPTION 'run this migration as an administrator, not runtime role %', session_user;
    END IF;
    IF to_regclass('shared.users') IS NULL OR NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'prod') THEN
        RAISE EXCEPTION 'shared/prod schema split must exist first';
    END IF;
    EXECUTE format('GRANT deck_chess_schema_owner TO %I', session_user);
END
$guard$;

SET LOCAL ROLE deck_chess_schema_owner;
CREATE TABLE IF NOT EXISTS prod.challenge_clears (
    user_id TEXT NOT NULL REFERENCES shared.users(id) ON DELETE CASCADE,
    challenge_id TEXT NOT NULL,
    first_cleared_at_ms BIGINT NOT NULL,
    PRIMARY KEY (user_id, challenge_id)
);
CREATE INDEX IF NOT EXISTS challenge_clears_user_first_cleared
    ON prod.challenge_clears (user_id, first_cleared_at_ms DESC);
REVOKE ALL ON prod.challenge_clears FROM PUBLIC, prod_app, test_app, deck_chess, deck_chess_test;
GRANT SELECT, INSERT ON prod.challenge_clears TO prod_app;

DO $verify$
BEGIN
    IF NOT has_table_privilege('prod_app', 'prod.challenge_clears', 'SELECT')
       OR NOT has_table_privilege('prod_app', 'prod.challenge_clears', 'INSERT')
       OR has_table_privilege('prod_app', 'prod.challenge_clears', 'DELETE')
       OR has_table_privilege('test_app', 'prod.challenge_clears', 'SELECT') THEN
        RAISE EXCEPTION 'prod.challenge_clears privilege verification failed';
    END IF;
END
$verify$;
RESET ROLE;

DO $cleanup$
BEGIN
    EXECUTE format('REVOKE deck_chess_schema_owner FROM %I', session_user);
END
$cleanup$;
COMMIT;
