BEGIN;
SELECT pg_advisory_xact_lock(hashtextextended('deck-chess-test-analysis-draws-v1', 0));
DO $admin_guard$ BEGIN
 IF session_user IN ('deck_chess','deck_chess_test') THEN RAISE EXCEPTION 'run as database administrator, not runtime role %',session_user; END IF;
 IF EXISTS (SELECT 1 FROM pg_auth_members m JOIN pg_roles r ON r.oid=m.roleid JOIN pg_roles u ON u.oid=m.member WHERE r.rolname='deck_chess_schema_owner' AND u.rolname=session_user) THEN RAISE EXCEPTION 'administrator % must not retain schema-owner membership',session_user; END IF;
 EXECUTE format('GRANT deck_chess_schema_owner TO %I',session_user);
END $admin_guard$;
SET LOCAL ROLE deck_chess_schema_owner;

ALTER TABLE test.game_analysis_nodes ADD COLUMN IF NOT EXISTS draws JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(draws) = 'array');
RESET ROLE;
DO $cleanup$ BEGIN EXECUTE format('REVOKE deck_chess_schema_owner FROM %I',session_user); END $cleanup$;
DO $cleanup_verify$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_auth_members membership
        JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
        JOIN pg_roles member_role ON member_role.oid = membership.member
        WHERE granted_role.rolname = 'deck_chess_schema_owner'
          AND member_role.rolname = session_user
    ) THEN
        RAISE EXCEPTION 'temporary schema-owner membership was not removed';
    END IF;
END
$cleanup_verify$;
COMMIT;
