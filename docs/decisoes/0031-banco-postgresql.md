# 0031 — Database: PostgreSQL

**Status:** decided · **Date:** 10/11/2026

## Context

The previous design used SQLite, in two files: a small and critical one, another with the messages. The choice came from the source service. The data model already foresaw the change: every access to the database goes through a repository layer.

## Decision

The hub uses PostgreSQL. In production it runs in a container, beside the hub, through the compose file. In development, a local PostgreSQL is used, with no container.

## Consequences

- One database only. The separation into two files existed because SQLite has one writer; in PostgreSQL, the messages stay in tables with purge by date ([15-mvp.md](../15-mvp.md), section 2).
- The queues stay in the database, with `FOR UPDATE SKIP LOCKED`. The hub is notified by `LISTEN/NOTIFY` and also checks from time to time.
- The master key and the audit mark stay outside the database and outside its backup ([04](../04-seguranca-conexao.md), C11).
- The backup becomes the PostgreSQL one. The rule of [03-arquitetura.md](../03-arquitetura.md), section 8, is rewritten with this.
- The database is one more service for whoever installs to take care of. In exchange, more than one process writes at the same time, which the admin command and, later on, a second hub need.
- Whoever only reads the database still cannot pass as a Hermes: nothing of what is kept has changed.
