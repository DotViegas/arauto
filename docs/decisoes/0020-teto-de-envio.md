# 0020 — Technical send ceiling

**Status:** decided · **Date:** 10/11/2026

## Context

An LLM in a loop can send hundreds of messages and get the number blocked. A ceiling in the hub avoids that, but the hub has no business rule ([0004](0004-o-hub-so-isola.md)).

## Decision

The hub has a send ceiling per Hermes and per number, with a fixed value per minute in the first version. Above it, the sending is refused with the error AR-704. The ceiling comes in as isolation: it protects a number from a Hermes out of control, as one neighbor is protected from another. The hub does not look at the content or at the recipient.

## Consequences

- The starting value, fixed in the first MVP, is 20 sends per minute ([15-mvp.md](../15-mvp.md)). It is calibrated with use.
- The alternative considered, a ceiling adjustable by the admin in the panel, does not go into the first version.
- The care about what to send stays in the instructions of each Hermes.
