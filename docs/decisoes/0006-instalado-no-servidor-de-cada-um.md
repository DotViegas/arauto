# 0006 — Installed on each one's own server

**Status:** decided · **Date:** 09/11/2026

## Context

A central hub would keep the chats and the engine sessions of everyone who uses it and would concentrate the risk in one place only.

## Decision

Arauto is installed on the server of whoever uses it, one organization per installation. The database is born separated by organization, so that a hub can serve several without a rewrite.

## Consequences

- Chats, engine sessions and keys stay with whoever installs.
- Every business table belongs to an organization from the first migration.
- With several organizations on a hub, one engine for each: never shared.
- Not built now: sign-up open to the public and usage limits per organization.
