# 0003 — Hub and engine are separate processes

**Status:** decided · **Date:** 09/11/2026

## Context

The WhatsApp engine is a third-party program. Merging it into the hub would force a restart of every engine session at each new version of the hub. Leaving it loose, configured by hand, allows a silent error: an engine session created with the wrong option discards history without warning.

## Decision

Hub and engine remain two processes. The hub is the only one that talks to the engine and is the owner of the configuration of each engine session: it creates, configures, checks and corrects.

## Consequences

- A new version of the hub does not bring WhatsApp down.
- The configuration of an engine session exists in one place only.
- Changing the version of the engine remains an act of whoever installs. The hub does not control containers.
- The hub needs a contract test against the engine at each new version.
