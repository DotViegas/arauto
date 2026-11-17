# 0049 — Waking by WhatsApp is decided by the verb "be woken" alone

**Status:** decided by the maintainer · **Date:** 16/11/2026. At his request it amends [0045](0045-a-marca-da-ligacao-basta-para-acordar.md) in three points: the role of the switch of the Hermes, the column and the audit event that "keep their names", and the meaning of the field `wake` of `session`. The rule of 0045 for notes does not change. **Built on 17/11/2026** and covered by the automatic tests, against a fake engine and a fake Hermes; not tried against a real Hermes or in a browser.

## What happened

The access editor of the panel shows two similar checkboxes: "Be woken", a verb of one access, and "Woken by WhatsApp", the switch of the Hermes for every number. A WhatsApp message wakes a Hermes only with both on. Since 0045 the switch no longer governs notes, so it only repeats, for the whole Hermes, what the verb already says per access. An admin who ticks "Be woken" and sees nothing happen has to find the second checkbox. The maintainer asked for one checkbox.

## Decision

- A WhatsApp message wakes a Hermes when the verb "be woken" holds for that chat: on the access of the number, or on the line of the chat when the chat has its own rule. Nothing else about the Hermes is consulted. The other conditions stay as they are: the kind of chat, the state of the number, an active device, the notice limits, the key of the local API of Hermes.
- The switch of the Hermes goes away: the checkbox "Woken by WhatsApp" of the access editor, the column of the list of Hermes, the line in the drawer of the org chart, the command `arauto-hub admin wake NAME on|off` and the column `hermes.can_be_woken` with its reason.
- In the access editor, turning "Be woken" off also turns it off in the chats of that access that have their own rule, in the same write, each with its audit event. Only that change does it: a save that leaves "Be woken" as it was, on or off, does not touch those lines, and neither does turning it on. A group wakes only by a line of its own, so a save that changes something else must not silence it. The command `admin access grant` keeps its behaviour and never touches the lines. The hint of the checkbox says so: "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule."
- **At the upgrade,** for every Hermes whose switch is off, a migration turns "be woken" off on its accesses and on the lines of its chats. No Hermes starts being woken by WhatsApp because of the upgrade. Before the upgrade the admin receives the list of what will be turned off, read from the database, to turn back on what is wanted.
- The audit event `hermes.wake_changed` is no longer written. It stays in the closed list of security events, because the audit only grows and old lines carry it.
- `arauto_status`: `woken_by.whatsapp` says that some access of this Hermes has the verb "be woken"; `can_be_woken` stays "notes or WhatsApp". The names do not change.
- The channel protocol keeps the field `wake` of `session`, a boolean, because every connector already installed requires it. The hub fills it with the same value as `woken_by.whatsapp`. The connector keeps using it only to show it, and its line "Waking by WhatsApp allowed by the admin: yes|no" keeps its words.

## Why this migration, when 0045 refused a similar one

0045 refused a migration that removed the wake marks of links, because it would undo what the admin had just drawn. Here the two cases differ in what the admin gets without asking. In 0045, leaving the marks made the upgrade do what the admin had drawn. Here, leaving the verbs would make the upgrade wake a Hermes that the admin had kept silent with the switch. Between undoing a setting that had no effect and starting an effect nobody asked for, the first is the one that can be seen and reversed in the panel.

## What is lost

- **The pause of one Hermes for every number at once.** To stop WhatsApp waking of a Hermes, the admin turns "be woken" off on each of its accesses, or revokes it.
- **The second step.** Granting "be woken" wakes from that moment. Until now a new Hermes was born with the switch off and needed `admin wake NAME on` as well.
- **Discarding by the switch.** Turning the switch off discarded the WhatsApp notices already waiting. From now on a waiting notice is discarded when the verb or the access is gone, or the number is not connected.
- **The verbs turned off at the upgrade.** The database does not keep which ones they were. The list given to the admin before the upgrade and the backup taken by `migrate` are the only records.

## What is not guaranteed

- **The migration leaves no audit event** for the verbs it turns off. The rule "every access change leaves an event with before and after" ([06](../06-modelo-de-dados.md)) holds for writes of the admin and of the channel. A migration is plain SQL and cannot reach the mark that every security event checks; writing these events would take a step in Rust after the migration, with an actor of its own and a rule for a restored database. It was not built. The list given before the upgrade is the record.
- **A chat with its own rule under an access without the verb.** When the access does not have "be woken" and a line of a chat does, the panel shows that the Hermes is woken there and the editor has no way to turn that line off: the checkbox is already off. It is turned off with `arauto-hub admin access chat`, or by turning "Be woken" on and off again in the editor.
- That the line of an already installed connector is fresh. `session` is sent once per channel session, so after an access changes the connector keeps showing the old value until it connects again. It was so with the switch too.
- `woken_by.whatsapp` says that some access has the verb; whether a given chat wakes still depends on its line, the kind of chat and the state of the number.
- How many accesses the migration turns off on a given installation: it depends on its data.
- Not tried against a real Hermes.

## Alternatives considered

- **Keep the switch in the database and make one checkbox write both.** The screen would look the same, but the panel would turn a security rule on and off by itself, and the value could drift from `admin wake`. 0045 refused a hidden write of this kind.
- **Keep both and rename the switch to a pause.** Two controls stay, which is what confuses.
- **Leave the verbs as they are at the upgrade.** Every Hermes with the switch off and the verb on would start being woken when the upgrade lands.
- **Turn off only the verb of the access at the upgrade.** The line of a chat prevails over the access, so those chats would start waking.
- **Leave the lines of the chats alone when "Be woken" is turned off in the editor.** The panel does not edit those lines, so the admin would have no way to stop them there.
- **Keep the column as a dead one.** A rule that nothing reads, waiting to be read by mistake.

## Consequences

To be done together with the code: a dated note in [03](../03-arquitetura.md), [06](../06-modelo-de-dados.md), [07](../07-telas.md), [08](../08-permissoes.md), [09](../09-contrato-mcp.md), [10](../10-maquinas-de-estado.md), [11](../11-conector.md), [13](../13-conversa-entre-hermes.md), [14](../14-protocolo-do-canal.md), [15](../15-mvp.md), the [glossary](../glossario.md) and decisions [0040](0040-painel-dentro-do-hub.md), [0043](0043-organograma-e-niveis.md) and [0045](0045-a-marca-da-ligacao-basta-para-acordar.md). [04](../04-seguranca-conexao.md) and the connection block of [02](../02-como-funciona.md) do not name the switch and do not change.

A new test fails when the rule is removed: a WhatsApp notice that waits is discarded when "be woken" is turned off, on the access and on the line of the chat. A test of the migration starts from a database with the switch off and the verb on, in both places.
