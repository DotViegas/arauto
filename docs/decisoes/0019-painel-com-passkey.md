# 0019 — Panel with passkey on every action that changes something

**Status:** decided · **Date:** 09/11/2026

## Context

The admin's account is the root of everything: whoever gets into it creates invites, approves devices and grants accesses. A password with a six-digit code falls to real-time phishing.

## Decision

The login of the panel is by passkey, and there is no password. Every action that changes something asks for the passkey again, at that moment: with the panel session alone, one can only look. The first admin is only created with the installation code, which the hub shows on its first start.

## Consequences

- A stolen panel session is not enough to act.
- The database keeps only public keys of the admins.
- Whoever loses the passkey receives another from a second admin; with no second admin, a command on the server issues a new installation code.
- The panel stays at an address separate from the port of the connectors and, by default, only on a private network ([0028](0028-painel-fechado-por-padrao.md)).
- Every text coming from a device or from WhatsApp is escaped on screen.
- The panel still depends on the TLS of whoever installs.
- **Change of 12/11/2026, at the maintainer's request:** while the passkey does not exist, the panel is entered with an entry code, and the panel session opened by it can act (connect Hermes, pair, give access). In that period the panel stays closed: its port is only reached on the hub's own machine, by tunnel. The passkey, at login and on every action that changes something, comes back into force before the panel is opened to the internet. What this leaves unprotected is in [0040](0040-painel-dentro-do-hub.md).
- **Change of 13/11/2026, at the maintainer's request:** the panel is entered with a passkey only, and the code is back to what this decision says: the installation code, which only opens the registration of a passkey (the first one, or another for who lost theirs). The note of 12/11/2026 above recorded a proposal of the first delivery of the panel, not a choice of the maintainer, and no longer holds. **What of this decision is not built:** asking for the passkey again on every action that changes something. Today the passkey is asked at the entry, and the panel session opened by it acts while it lasts; what that leaves unprotected is in the 0040.
