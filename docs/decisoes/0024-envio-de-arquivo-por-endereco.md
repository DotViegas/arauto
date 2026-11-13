# 0024 — Sending a file by address

**Status:** decided · **Date:** 10/11/2026

## Context

A tool that sends a file from an address makes the hub fetch something chosen by someone else. The source service solves this by downloading it itself, and only from a public address.

## Decision

Keep the sending by address, as in the source: who downloads is the hub, and only from a public address. The sending by content continues to exist beside it.

## Consequences

- The hub needs to refuse an address that leads to the internal network, also after a redirect and after the resolution of the name. The rule comes with a test, in Phase 2 of the [roadmap](../12-roteiro.md).
- The alternative considered, accepting only content in the first version, would eliminate this class of attack, at the cost of splitting large files into parts over the channel.
