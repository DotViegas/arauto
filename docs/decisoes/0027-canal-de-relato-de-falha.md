# 0027 — Channel for reporting a flaw

**Status:** decided · **Date:** 10/11/2026

## Context

Whoever finds a security flaw needs a reserved path to report it. A public issue hands the flaw to everyone before the fix.

## Decision

The channel is GitHub's private vulnerability reporting, in the security tab of the repository. There is no e-mail inbox for this.

## Consequences

- The public repository stays on GitHub.
- The channel comes into existence when the repository is opened ([roadmap](../12-roteiro.md), Phase 7). The [SECURITY.md](../../SECURITY.md) points to it.
- The maintainer needs to turn on private reporting in the settings of the repository and follow the notifications.
