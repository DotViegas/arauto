# 0010 — Identity created on the device

**Status:** decided · **Date:** 09/11/2026

## Context

If the sealed credential were the only thing that identifies a Hermes, whoever got the master key of the hub from a backup could create credentials and pass as any Hermes.

## Decision

Besides the sealed credential, the connector creates a key pair on its own machine. The private part never leaves it. The hub requires both.

## Consequences

- A stolen copy of the hub is not enough to pass as a Hermes.
- The hub keeps only the public part.
- The path of whoever uses it does not change.
