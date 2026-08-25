# Legacy SQLx migration history

Files in this directory describe the pre-isolation `public` schema and are kept
unchanged so an already-applied SQLx migration history retains its checksums.
The application no longer embeds or runs this directory, and deployment must
not run `sqlx migrate run` against it.

In particular, `20260824000000_game_records.sql` is historical only. Running it
now would create an unqualified `public.game_records` table. New and upgraded
environments must use the approved releases in `../db/prod` or `../db/test`.
