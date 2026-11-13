# 0004 — The hub only isolates

**Status:** decided · **Date:** 09/11/2026

## Context

It would be easy to put rules of conduct in the hub: hours, forbidden words, service queue, approval of sending. Each of them is an opinion about how an agent should behave.

## Decision

The hub has no business rule. The only thing it imposes is isolation: a Hermes only reaches what it was given. The conduct belongs to Hermes.

## Consequences

- The hub does not filter content, does not answer on its own and does not choose which Hermes acts.
- When two Hermes overlap, the panel warns and does not prevent.
- Purely technical limits need to be justified as isolation ([0020](0020-teto-de-envio.md)).
