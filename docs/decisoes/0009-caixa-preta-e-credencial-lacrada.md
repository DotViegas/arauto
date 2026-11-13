# 0009 — Vault in the hub and sealed credential

**Status:** decided · **Date:** 09/11/2026

## Context

Once connected, Hermes needs to come back on its own when it restarts. For that it has to keep something. A fixed key that is kept serves forever whoever copies it.

## Decision

Who generates is the hub, in a part called the vault, by a draw of the system. Hermes keeps a sealed credential, which it cannot open. Every connection starts by swapping the credential for another, and the channel session only serves after the swap is confirmed. Using a copy leads to quarantine.

## Consequences

- Hermes connects once and comes back on its own.
- To use the hub it is necessary to swap the credential, and swapping kills the previous one. That is why whoever uses a copy is discovered when the true Hermes connects.
- With the true Hermes on, the copy is barred in seconds. With it off, the copy works until it comes back: there is no time limit.
- An old copy gives no access, but presenting it puts the device in quarantine. The connector's folder must stay out of the backups.
- The only way out of quarantine is a new invite. There is no "release".
- After the first confirmed one, the hub never issues a credential by identity alone.
- The master key of the vault stays outside the database and is one of the items to protect.
