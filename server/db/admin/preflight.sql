-- Read-only release preflight. This runs before any ordered migration.
DO $preflight$
BEGIN
    IF current_database() <> 'deck_chess' THEN
        RAISE EXCEPTION 'expected database deck_chess, connected to %', current_database();
    END IF;
    IF session_user IN ('deck_chess', 'deck_chess_test', 'prod_app', 'test_app') THEN
        RAISE EXCEPTION 'database release requires an administrative login, got %', session_user;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess_schema_owner')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'deck_chess_test')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'prod_app')
       OR NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'test_app') THEN
        RAISE EXCEPTION 'required schema-owner and runtime roles are not provisioned';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'shared')
       OR NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'prod')
       OR NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'test')
       OR NOT EXISTS (
           SELECT 1 FROM pg_class relation
           JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
           WHERE namespace.nspname = 'shared' AND relation.relname = 'users'
             AND relation.relkind IN ('r', 'p')
       )
       OR NOT EXISTS (
           SELECT 1 FROM pg_class relation
           JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
           WHERE namespace.nspname = 'shared' AND relation.relname = 'auth_identities'
             AND relation.relkind IN ('r', 'p')
       ) THEN
        RAISE EXCEPTION 'split shared/prod/test schema bootstrap is incomplete';
    END IF;
END
$preflight$;

SELECT current_database() AS database_name,
       session_user AS administrative_login,
       'shared.users and shared.auth_identities found through pg_catalog' AS object_status;
