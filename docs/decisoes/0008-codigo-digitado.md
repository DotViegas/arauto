# 0008 — The admin types the verification code

**Status:** decided · **Date:** 09/11/2026

## Context

Whoever gets the invite before its use manages to create a request with the right name. If approving were a click in front of data that the device itself declares, that request would be approved.

## Decision

The connector computes and shows a six-digit verification code. Hermes says it in the command channel. The admin types it in the panel. The panel never shows the code.

## Consequences

- Only the request whose code the true Hermes said is approved.
- An invite leaked before its use yields, at most, getting in the way of the connection.
- What the device declares appears as unverified and decides nothing.
- Five errors close the request; the deadline is 10 minutes.
- If the command runs again, the most recent code is the one that counts.
