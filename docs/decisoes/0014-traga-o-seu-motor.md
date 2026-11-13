# 0014 — Bring your own engine

**Status:** proposal, to be validated in the prototype · **Date:** 09/11/2026

## Context

The engine used today is third-party, is an unofficial WhatsApp client, and the image that was checked declares an undefined license.

## Decision

Arauto does not embed or redistribute the engine. Whoever installs gives the address and the key of their own engine. The engine stays behind an interface, so that another one, including the official API, can come in as an adapter.

## Consequences

- The license, the cost and the risk of the engine belong to whoever installs it, and the project says so clearly.
- The hub checks the version of the engine at start and reports when it is not a tested version.
- A second adapter can be added without touching the core.
