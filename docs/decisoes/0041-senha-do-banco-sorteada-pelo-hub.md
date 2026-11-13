# 0041 — The password of the database, drawn by the hub

**Status:** proposal · **Date:** 13/11/2026, with the code

## Context

On 13/11/2026 the maintainer asked for an installation done entirely on the screens of a web panel
for servers (Easypanel), with the first configuration in the panel of the hub. A composition that
the panel of the server starts brings PostgreSQL with a password written in the composition
itself: the same for everyone who installs. The project's principle 2 says a long-lived secret
never passes through a person.

## Decision

- **The hub can replace the password of its own database role with one nobody sees.** It draws
  32 bytes from the system generator, writes them as 64 hexadecimal characters in
  `database.secret`, in its data folder (a private file, written durably), runs
  `ALTER ROLE ... PASSWORD` and, from then on, opens the database with the address of
  `ARAUTO_DATABASE_URL` with the password of that file in its place.
- **When.** By the command `arauto-hub admin database rotate-password`, at any time; and by
  itself only when `ARAUTO_DATABASE_PASSWORD=rotate-at-first-access` is set, once, right after the
  first passkey of the installation is registered in the panel. With the file already there, the
  automatic path does nothing.
- **In an order that survives a stop at any point.** The new password goes first to
  `database.secret.next`; the statement and its audit event are one security write; only then
  the file takes its place. A hub that starts and finds `database.secret.next` tries the password
  in use and, only when PostgreSQL refuses it, the new one, and keeps whichever works. A lock file
  keeps two replacements from running at once.
- **The running hub does not restart.** Its pools of connections take the new password for every
  connection they open from then on. Another hub process that was already running does not: it
  needs a restart (the command says so).
- **In the audit:** `database.password_replaced`, a security event, with no secret and no digest
  of it.

## What is not guaranteed

- **The password travels to PostgreSQL in the statement.** If the server logs statements
  (`log_statement` = `ddl` or `all`), or a failed statement (`log_min_error_statement`), the
  password is written in the log of the server. Sending the SCRAM verifier already computed, as
  `psql` does, would avoid it: not done.
- Between the first start and the first access, the database has the password of the composition.
  Only what is on the network of the composition reaches it in that interval.
- The data folder holds the password of the database beside the master key. Whoever takes that
  folder takes both; losing it loses both. The way back from a lost password file is an
  `ALTER ROLE` by a superuser of PostgreSQL and removing the file.
- A stop in the middle of a durable write may leave a temporary file with the password in the data
  folder; the hub does not clean those (the same is true of the master key). On Windows the folder
  is not forced to disk after a rename.
- Inside the library that opens the connections, the password is kept in memory without being
  wiped when it is dropped.
- A database that accepted any password (trust) and then starts to require one may lead the hub
  to discard a new password the database had taken.
- Tested against the test PostgreSQL with a role of its own for each test. That server accepts any
  password on this machine, so the branch "PostgreSQL refused the password in use" was proved only
  by a pure function, not by a connection.

## Consequences

- The variables of the hub gain `ARAUTO_DATABASE_PASSWORD`. Any other value than
  `rotate-at-first-access` is a configuration error.
- The data folder gains `database.secret` and, at moments, `database.secret.next` and
  `database.secret.lock`. Its backup, apart from the one of the database, now carries the password
  too.
- The recipe of Easypanel ([deploy/easypanel](../../deploy/easypanel/README.md)) uses it. The
  composition of Docker Compose does not: its password is drawn by whoever installs, in a file
  that never enters the repository.
- Alternatives considered: the password typed by the admin in the panel (it would pass through a
  person and a screen); a password drawn by the panel of the server (not every panel does it, and
  it would be shown on its screens); restarting the whole composition after the change (the hub
  does not need it).
