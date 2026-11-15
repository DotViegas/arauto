# 0045 — The wake mark of a link is enough to wake by a note

**Status:** decided by the maintainer · **Date:** 15/11/2026. At his request it amends the rule, written in [13](../13-conversa-entre-hermes.md) (sections 2, 3 and 5) and [08](../08-permissoes.md) (section 4), that a note wakes only a Hermes that "can be woken". No other decided rule changes.

## What happened

In a trial on 15/11/2026 the admin drew links marked "wakes" on the org chart. A Hermes sent a note; the note was kept and the receiver was not woken, because the switch of the receiver (`arauto-hub admin wake NAME on|off`) was off. The org chart only showed that switch, and a Hermes that read `"can_be_woken": false` concluded that no note could wake it.

## Decision

- Between Hermes, the wake mark of the link is enough to wake the receiver. The switch of the Hermes is not consulted for notes, neither when the note is kept nor when its notice leaves.
- The switch governs only waking by WhatsApp messages, together with the verb "be woken" of the access, as before. In words it is "woken by WhatsApp". It is set in the access editor of the panel and by `arauto-hub admin wake NAME on|off`.
- The database does not change: the column `hermes.can_be_woken`, the audit event `hermes.wake_changed` and its flag keep their names; only their scope narrows.
- `arauto_status` says what can wake the Hermes: `woken_by.notes`, `woken_by.whatsapp`, and `can_be_woken` when either is true.
- The channel protocol does not change. The field `wake` of `session` still carries the switch, now described as WhatsApp only. The connector never used it to decide whether to wake; it only shows it.

## What still holds for a note to wake

The link from the sender to the receiver exists at the moment of the note and when the notice leaves, with the mark, in that direction, inside the organization; both Hermes are live; the receiver has an active device; the note limits; one pending notice per sender; 10 notices per hour per link and 30 per hour per receiver, beyond which the note is kept without notice; 10 seconds between two notices of the same pair; the connector's 30 wakes per minute and 300 per hour; the key of the local API of Hermes.

## What is lost

- The single switch that silenced a Hermes entirely. To stop note wakes, the admin removes the mark from each link that reaches the Hermes (the arrow on the org chart, or `arauto-hub admin link add FROM TO` without `--wake`), or removes the link. Revoking the Hermes or leaving it without a device still stops every wake.
- At the upgrade, every link that already has the mark starts waking its receiver, even when that receiver's switch is off. Notes kept before the upgrade wake nobody.

## What is not guaranteed

- The limits bound a loop; they do not stop waste. Two Hermes with marked links both ways can wake each other up to 10 times an hour in each direction, and each wake is a run of the Hermes.
- One mark is now enough: a publish of the org chart, or `admin link add --wake --both`, makes a pair that can wake each other with no second step.
- A connector of an older version keeps printing "Waking allowed by the admin" with the switch, while it wakes on notes with the switch off.
- `woken_by.whatsapp` says that some access has the verb; whether a given chat wakes still depends on its line, the kind of chat and the state of the number.
- Not tried against a real Hermes.

## Alternatives considered

- Keep the rule and show the switch on the org chart: two switches for one intent; refused by the maintainer.
- Turn the switch on automatically when a mark is set: a hidden write of a security rule.
- A migration that removes the marks of links to Hermes whose switch is off: it would undo the links the admin had just drawn.
- Rename the column: a migration with no gain in behaviour.

## Consequences

The documents [03](../03-arquitetura.md), [07](../07-telas.md), [08](../08-permissoes.md), [09](../09-contrato-mcp.md), [11](../11-conector.md), [13](../13-conversa-entre-hermes.md), [14](../14-protocolo-do-canal.md), [15](../15-mvp.md), the [glossary](../glossario.md) and decisions [0040](0040-painel-dentro-do-hub.md) and [0043](0043-organograma-e-niveis.md) carry a dated note.
