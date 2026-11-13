# 0007 — Single-use invite, run by Hermes's LLM

**Status:** decided · **Date:** 09/11/2026

## Context

Connecting an agent required administrator access to the server. The simple alternative, a fixed key pasted into the chat, would leave a permanent secret in the conversation history.

## Decision

A Hermes is connected by pasting a block into its command channel. The block carries an invite that is valid for 15 minutes and one use, and that only serves to ask for a connection. Who runs it is Hermes's LLM.

## Consequences

- No long-lived secret goes through the chat.
- Opening an address never spends the invite; only the enrollment spends it.
- Another device that presents the invite while the request is pending is refused, and the panel alerts.
- Presenting an invite that is already closed only generates a record. It never brings down a connected Hermes.
- The gesture of pasting a block can be imitated by a scam: hence [0008](0008-codigo-digitado.md) and [0017](0017-bloco-sem-script-da-internet.md).
