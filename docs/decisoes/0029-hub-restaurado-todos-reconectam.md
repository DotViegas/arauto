# 0029 — Hub restored from a backup: everyone reconnects

**Status:** decided · **Date:** 10/11/2026

## Context

A hub that comes back from an old backup forgets what was revoked, swapped or removed after it. What had already stopped being valid becomes valid again in the database.

## Decision

On noticing the restoration, the hub suspends all active devices, and each Hermes comes back with a new invite. There is no "reactivate". The alternative considered, reactivating each device with a click, would save work after a disaster, but would ask the admin to judge device by device, and a mistake would give access back to a copy.

## Consequences

- After a restoration, each Hermes needs a new invite. The accesses come back as they were in the backup: what changed after it was lost and needs to be checked (C11).
- The complete rule is in [04-seguranca-conexao.md](../04-seguranca-conexao.md), C11. The way the hub notices the restoration is still a proposal, to be validated in Phase 1 of the [roadmap](../12-roteiro.md).
- What remains open is at the end of C11.
- On 11/11/2026 the hub already notices the restoration by the two ways of C11 (the mark, at start and at each sweep, and the seal the database does not explain) and turns the mode on, with a test, including with the hub running. Two points of the code still wait for a decision: the unknown one with the mode on and the unknown one again after the purge ([14](../14-protocolo-do-canal.md), section 13).
