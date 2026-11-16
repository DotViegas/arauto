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

## Note of 15/11/2026

The first path (the entry references Hermes's variable) was tried by hand on **one** Hermes: Hermes Agent v0.22.5, Docker install, Linux. It worked: Hermes resolved `${API_SERVER_KEY}` in the entry's environment, the connector received the key, and a note woke that Hermes ([referencia/hermes-fatos.md](../../referencia/hermes-fatos.md), K1 to K4). The second path is **not needed for now and not built**.

**Not proved:** Windows, macOS, v0.23 and other versions of Hermes; the shim `/opt/hermes/bin/hermes` passing an argument with `${...}` unchanged. **New exposure:** the key now sits in the initial environment of each connector copy. For the Hermes without a key and for the rest of this change, see [0046](0046-chave-pela-entrada-e-hermes-sem-chave.md). This decision stays as decided.

## Note of 16/11/2026

At the maintainer's request, a Hermes without a key is no longer always left without waking: in `join`, when Hermes has no key and the conditions of [0046](0046-chave-pela-entrada-e-hermes-sem-chave.md) hold, the connector draws one and saves it with Hermes's own command. The key still never goes to the hub and never appears on screen; while that command runs, it sits in the command's arguments (0046). The decision above stays as decided.
