\set ON_ERROR_STOP on

\echo 'Administrative migration preflight'
SELECT current_database() AS database, current_user, session_user;

SELECT current_database() = 'deck_chess' AS database_ok,
       current_user = session_user AS unaltered_role_ok,
       session_user NOT IN ('deck_chess', 'deck_chess_test') AS admin_login_ok,
       NOT EXISTS (
           SELECT 1
           FROM pg_auth_members membership
           JOIN pg_roles granted_role ON granted_role.oid = membership.roleid
           JOIN pg_roles member_role ON member_role.oid = membership.member
           WHERE granted_role.rolname IN ('deck_chess', 'deck_chess_schema_owner')
             AND member_role.rolname = session_user
       ) AS membership_clean_ok,
       NOT EXISTS (
           SELECT required.rolname
           FROM (VALUES
               ('deck_chess'),
               ('deck_chess_test'),
               ('deck_chess_schema_owner'),
               ('prod_app'),
               ('test_app')
           ) required(rolname)
           WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE pg_roles.rolname = required.rolname)
       ) AS roles_ok,
       NOT EXISTS (
           SELECT required.nspname
           FROM (VALUES ('shared'), ('prod'), ('test')) required(nspname)
           WHERE NOT EXISTS (
               SELECT 1 FROM pg_namespace WHERE pg_namespace.nspname = required.nspname
           )
       ) AS schemas_ok,
       NOT EXISTS (
           SELECT required.nspname, required.relname
           FROM (VALUES
               ('shared', 'users'),
               ('shared', 'auth_identities')
           ) required(nspname, relname)
           WHERE NOT EXISTS (
               SELECT 1
               FROM pg_class relation
               JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
               WHERE namespace.nspname = required.nspname
                 AND relation.relname = required.relname
                 AND relation.relkind IN ('r', 'p')
           )
       ) AS shared_objects_ok,
       EXISTS (
           SELECT 1
           FROM pg_class relation
           JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
           JOIN pg_roles owner_role ON owner_role.oid = relation.relowner
           WHERE namespace.nspname = 'shared'
             AND relation.relname = 'users'
             AND relation.relkind IN ('r', 'p')
             AND owner_role.rolname = 'deck_chess'
       ) AS users_owner_ok
\gset preflight_

\if :preflight_database_ok
  \echo '  database deck_chess                         OK'
\else
  \echo 'ERROR: connected database must be deck_chess'
  \quit 1
\endif
\if :preflight_unaltered_role_ok
  \echo '  current_user/session_user                  OK'
\else
  \echo 'ERROR: preflight requires an unaltered administrative session role'
  \quit 1
\endif
\if :preflight_admin_login_ok
  \echo '  non-runtime administrative login           OK'
\else
  \echo 'ERROR: runtime logins cannot run migrations'
  \quit 1
\endif
\if :preflight_membership_clean_ok
  \echo '  permanent owner membership absent          OK'
\else
  \echo 'ERROR: administrator has deck_chess or deck_chess_schema_owner membership'
  \quit 1
\endif
\if :preflight_roles_ok
  \echo '  required roles                             OK'
\else
  \echo 'ERROR: one or more required roles are missing'
  \quit 1
\endif
\if :preflight_schemas_ok
  \echo '  shared/prod/test schemas                   OK'
\else
  \echo 'ERROR: split schemas are not fully provisioned'
  \quit 1
\endif
\if :preflight_shared_objects_ok
  \echo '  shared.users/auth_identities               OK'
\else
  \echo 'ERROR: required shared tables are missing'
  \quit 1
\endif
\if :preflight_users_owner_ok
  \echo '  shared.users owner deck_chess              OK'
\else
  \echo 'ERROR: shared.users must be owned by deck_chess'
  \quit 1
\endif
