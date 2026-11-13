# 0028 — Panel closed by default; whoever installs chooses

**Status:** decided · **Date:** 10/11/2026

## Context

The admin's account is the root of everything ([0019](0019-painel-com-passkey.md)). A panel reachable from the internet is exposed to whoever scans addresses. But there are those who need to administer from outside and have no private network.

## Decision

Whoever installs chooses. The default installation comes with the panel closed: it only answers on the private network or through a VPN. The installation guide shows how to open it to the internet, at the responsibility of whoever installs. The port of the connectors is the only one that needs to be reachable from outside.

## Consequences

- Panel and connectors stay at separate addresses ([03-arquitetura.md](../03-arquitetura.md), section 4).
- Open or closed, the panel requires the passkey on every action that changes something and depends on the TLS of whoever installs.
- The installation guide ([roadmap](../12-roteiro.md), Phase 7) states the risk of opening.
