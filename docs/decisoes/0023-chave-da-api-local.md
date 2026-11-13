# 0023 — How the connector gets the key of Hermes's local API

**Status:** decided · **Date:** 10/11/2026

## Context

To wake Hermes, the connector calls its local API, which requires a key. The key is in Hermes's secrets file, and the connector, as a child process, does not receive it from the environment.

## Decision

The paths are tried in this order. First, the entry of the tool server references Hermes's variable, and it is Hermes itself that hands the key to the connector. If the test shows that this does not work, the connector reads the key from Hermes's secrets file, with a clear warning to whoever installs. Without a key, that Hermes is left without waking.

## Consequences

- The test of the first path belongs to Phase 1 of the [roadmap](../12-roteiro.md). The verb "be woken" depends on it (Phase 5).
- If the second path holds, the text of the block comes to say that the connector reads that file.
- On either path, the key never goes to the hub and never appears on screen.
