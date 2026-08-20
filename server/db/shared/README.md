# Shared migrations

Account identity, profile, rating and achievement migrations belong here. They are
administrative releases and must never run as part of a prod or test application
deployment. The initial move from `public` is in `../admin`.

Do not put decks or custom-piece tables in this schema.
