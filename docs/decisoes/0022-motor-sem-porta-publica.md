# 0022 — Engine with no public port, watched by the hub

**Status:** decided · **Date:** 09/11/2026

## Context

The engine has its own API and its own panel. Whoever reaches them configures an extra destination for the events and copies every message without going through invite, code or access.

## Decision

The engine stays only on the internal network, with a key of its own for the installation. The hub checks the configuration of each engine session at start and every 5 minutes; if it finds an event destination that is not the hub, it corrects and alerts.

## Consequences

- The Arauto panel replaces the engine's panel.
- The volume of the engine sessions stays out of the common backup: whoever has it has the WhatsApp.
- Pairing, restarting and removing numbers only through the panel, by an admin.
