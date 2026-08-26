# Shared migrations

Account identity, profile, rating and achievement migrations belong here. They are
administrative releases and must never run as part of a prod or test application
deployment. The initial move from `public` is in `../admin`.

Do not put decks or custom-piece tables in this schema.

Apply `20260826000000_profile_visibility.sql` as the database administrator
before deploying an application revision that reads `profile_visibility`. The
release is transactional, keeps existing accounts public, and grants only the
new column update privilege to `prod_app` and `test_app`.

`shared.users` remains owned by `deck_chess`. The script therefore temporarily
grants that role to the administrative session, performs the change as the
table owner, and removes the membership before commit. Do not pre-grant that
membership or run the script as either runtime login.
