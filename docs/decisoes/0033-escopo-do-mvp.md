# 0033 — Scope of the first MVP: conversation and WhatsApp together

**Status:** decided · **Date:** 10/11/2026

## Context

The roadmap foresaw eight phases in sequence, with the proofs before any code and the conversation between Hermes for after publication. The maintainer of the project asked to start the first MVP, with the conversation between Hermes and WhatsApp in the same batch.

## Decision

The first MVP brings together the connection, the conversation between Hermes, WhatsApp and waking, in five milestones ([15-mvp.md](../15-mvp.md)). Conversation and WhatsApp are built together, on the same connection.

Three points are a proposal of whoever writes, to be confirmed by the maintainer:

- **Administration by command on the server**, with no administration address on the network. The panel, with passkey ([0019](0019-painel-com-passkey.md)), comes after the MVP.
- **Code written from scratch.** The source service is not copied while the release by whoever has rights over it has not come ([roadmap](../12-roteiro.md), Phase 2). The ideas that come from it are already in these documents.
- **The "in the channel" proofs of Phase 1 are done inside the MVP**, as tests with randomized rounds. The "in Hermes" proofs remain pending and depend on a real Hermes.

## Consequences

- The roadmap is rewritten in the order of the milestones. The done criterion of each phase still holds.
- Left out of the MVP: panel, message archive, signing of the versions, sending a file by address and shielded mode. Restored mode goes in, because it touches every write of a security event from the first milestone.
- The design of the MVP went through an adversarial review on 10/11/2026, before the code. The corrections are in documents 13, 14 and 15.
- While there is no panel, the rule of the passkey on every action that changes something has nowhere to be applied: whoever administers is whoever has access to the server.
- The MVP is not a published version. Nothing is opened to the public before Phase 7.
- **Change of 12/11/2026, at the maintainer's request:** the panel started before the end of the MVP, with the
  read-only screens and the guide for connecting Hermes. How it goes into the hub, and what is still left to
  decide, is in [0040](0040-painel-dentro-do-hub.md), which is a proposal. On 13/11/2026 the
  editing of accesses and revoking came in; pairing numbers is still done by the command.
