# 0021 — Inactivity deadline

**Status:** decided · **Date:** 10/11/2026

## Context

The use of a copy of the credential is only discovered when the true Hermes comes back. A Hermes turned off forever and never revoked never comes back.

## Decision

An active device that goes 30 days without connecting becomes expired and only comes back with Reconnect, by a new invite. The accesses of the Hermes are kept.

## Consequences

- It catches the copy used for the first time after the deadline.
- It does not catch the copy that is already in use: it connects as the device, and the deadline never expires.
- The alternative that would close the whole case, a maximum validity of the device, was not adopted: it would cost reconnecting from time to time.
- A device expired by inactivity that presents a credential older than the last one generates the alert of reuse after closing, and the reconnect shows its last use ([04-seguranca-conexao.md](../04-seguranca-conexao.md), C7 and C10). Without this, the use of a copy would stop showing up after the deadline.
- The device expired by inactivity receives the error AR-507 ([11-conector.md](../11-conector.md), section 8). Its label in the panel is still to be written.
