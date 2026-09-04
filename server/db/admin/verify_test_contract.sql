-- Read-only postflight contract for APP_ENV=test/local. All object inspection
-- uses pg_catalog because the administrative login intentionally has no
-- permanent USAGE on application schemas.
DO $contract$
DECLARE
    challenge_table oid;
    game_records_table oid;
    analysis_trees_table oid;
    analysis_nodes_table oid;
BEGIN
    IF EXISTS (
        SELECT 1
        FROM (VALUES
            ('shared', 'users'),
            ('shared', 'auth_identities'),
            ('test', 'custom_piece_versions'),
            ('test', 'custom_piece_images'),
            ('test', 'game_records'),
            ('test', 'game_analysis_trees'),
            ('test', 'game_analysis_nodes'),
            ('test', 'challenge_clears')
        ) required(schema_name, table_name)
        WHERE NOT EXISTS (
            SELECT 1 FROM pg_class relation
            JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
            WHERE namespace.nspname = required.schema_name
              AND relation.relname = required.table_name
              AND relation.relkind IN ('r', 'p')
        )
    ) THEN
        RAISE EXCEPTION 'test runtime contract is missing required tables';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM (VALUES
            ('shared', 'users', 'profile_visibility'),
            ('test', 'game_records', 'white_user_id'),
            ('test', 'game_records', 'black_user_id'),
            ('test', 'game_records', 'retention_mode'),
            ('test', 'game_records', 'expires_at_ms'),
            ('test', 'challenge_clears', 'user_id'),
            ('test', 'challenge_clears', 'challenge_id'),
            ('test', 'challenge_clears', 'first_cleared_at_ms')
        ) required(schema_name, table_name, column_name)
        WHERE NOT EXISTS (
            SELECT 1 FROM pg_attribute attribute
            JOIN pg_class relation ON relation.oid = attribute.attrelid
            JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
            WHERE namespace.nspname = required.schema_name
              AND relation.relname = required.table_name
              AND attribute.attname = required.column_name
              AND attribute.attnum > 0 AND NOT attribute.attisdropped
        )
    ) THEN
        RAISE EXCEPTION 'test runtime contract is missing required columns';
    END IF;

    SELECT relation.oid INTO STRICT challenge_table
    FROM pg_class relation
    JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
    WHERE namespace.nspname = 'test' AND relation.relname = 'challenge_clears'
      AND relation.relkind IN ('r', 'p');

    SELECT relation.oid INTO STRICT game_records_table FROM pg_class relation JOIN pg_namespace namespace ON namespace.oid=relation.relnamespace WHERE namespace.nspname='test' AND relation.relname='game_records';
    SELECT relation.oid INTO STRICT analysis_trees_table FROM pg_class relation JOIN pg_namespace namespace ON namespace.oid=relation.relnamespace WHERE namespace.nspname='test' AND relation.relname='game_analysis_trees';
    SELECT relation.oid INTO STRICT analysis_nodes_table FROM pg_class relation JOIN pg_namespace namespace ON namespace.oid=relation.relnamespace WHERE namespace.nspname='test' AND relation.relname='game_analysis_nodes';

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint constraint_record
        WHERE constraint_record.conrelid = challenge_table
          AND constraint_record.contype = 'p'
          AND pg_get_constraintdef(constraint_record.oid) = 'PRIMARY KEY (user_id, challenge_id)'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint constraint_record
        JOIN pg_class referenced ON referenced.oid = constraint_record.confrelid
        JOIN pg_namespace namespace ON namespace.oid = referenced.relnamespace
        WHERE constraint_record.conrelid = challenge_table
          AND constraint_record.contype = 'f'
          AND namespace.nspname = 'shared' AND referenced.relname = 'users'
          AND constraint_record.confdeltype = 'c'
    ) THEN
        RAISE EXCEPTION 'test challenge clear key contract is invalid';
    END IF;
    IF NOT has_schema_privilege('test_app', 'shared', 'USAGE')
       OR NOT has_schema_privilege('test_app', 'test', 'USAGE')
       OR has_schema_privilege('test_app', 'prod', 'USAGE')
       OR NOT has_table_privilege('test_app', challenge_table, 'SELECT')
       OR NOT has_table_privilege('test_app', challenge_table, 'INSERT')
       OR has_table_privilege('test_app', challenge_table, 'UPDATE')
       OR has_table_privilege('test_app', challenge_table, 'DELETE') THEN
        RAISE EXCEPTION 'test runtime role privileges violate the environment contract';
    END IF;
    IF NOT has_table_privilege('test_app', game_records_table, 'DELETE')
       OR NOT has_table_privilege('test_app', analysis_trees_table, 'SELECT,INSERT,UPDATE,DELETE')
       OR NOT has_table_privilege('test_app', analysis_nodes_table, 'SELECT,INSERT,UPDATE,DELETE')
       OR NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid=analysis_trees_table AND confrelid=game_records_table AND confdeltype='c')
       OR NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid=analysis_nodes_table AND confrelid=analysis_trees_table AND confdeltype='c') THEN
        RAISE EXCEPTION 'test analysis/retention privilege or cascade contract is invalid';
    END IF;
    IF NOT pg_has_role('deck_chess_test', 'test_app', 'MEMBER')
       OR pg_has_role('deck_chess_test', 'prod_app', 'MEMBER') THEN
        RAISE EXCEPTION 'test login role membership violates the environment contract';
    END IF;
END
$contract$;

SELECT 'test' AS verified_environment,
       'runtime table, key, and privilege contract passed' AS contract_status;
