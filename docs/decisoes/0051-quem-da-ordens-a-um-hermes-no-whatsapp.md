# 0051 — The chat where a Hermes takes orders on WhatsApp is refused for writing

**Status:** proposal · **Date:** 17/11/2026. Asked by the maintainer. **Built on 17/11/2026** in three parts, A0, A1 and B, and covered by automatic tests against a fake engine; **not tried on a real Hermes or on a real WhatsApp, and part B was not tried in a browser**, the window of part B was rebuilt as option B on 18/11/2026 (the numbers of the hub chosen by alias, chosen by the maintainer), also tested against a fake engine only and **not tried in a browser**, and the texts were not approved by him word for word. Part B (the panel) is built and tested against a fake engine only; parts C and later (the warning at grant time, the connector report) **do not exist**; they are not described here as built.

## Asked by the maintainer (17/11/2026)

- When the WhatsApp of an operator is in Arauto, no Hermes can send **orders** through it to another Hermes. It can still send messages at the request of any Hermes: a Hermes may pass for the person to speed up the service.
- There is no "command number". Three numbers had been marked "command" by mistake, because the name did not say whether such a number gives orders or takes them. The mark goes, with no carry-over, and every new name says the direction.
- In `self-chat` mode, what is refused is the chat of the account with itself, not the whole number.
- Reading is not refused, not even in the chat where orders are given.
- If the authorized numbers cannot be pulled automatically, the admin informs which are blocked from sending, and the importance of the block must be shown.

## Decision (proposal)

**What is protected is a chat**, not a number: the command channel of a Hermes H on WhatsApp. It is the chat between the telephone H's own WhatsApp bridge runs on and a telephone that gives H orders (in `self-chat` mode, the chat of that account with itself). When one end of that chat is a number of the hub, the hub refuses every call that writes in it, for every Hermes, H included. Reading follows the accesses. Every other chat of the number stays as the accesses say.

- **What is recorded, per Hermes, typed by the admin.** (1) The telephone H's own WhatsApp runs on ("takes orders there"); empty means none. (2) Who gives H orders: its own WhatsApp in the chat with itself (`self-chat`) **or** a list of telephones, at most 16. A list needs the first control, because a chat has two ends. The telephones are sealed (a new field, `HermesWhatsappPhone`) and found by the keyed digest of each form, the one used for the telephone of a number. A telephone is in no log, audit row or tool answer.
- **What is refused.** `send_text`, `send_file` and `forward` whose destination is that chat (by hub identifier or by telephone, in both Brazilian forms, existing or not), `mark_read`, `set_typing`, and `react`, `edit_message` and `delete_message` on a message of that chat. Not refused: `check_number`, every reading, `forward` **from** that chat, being woken.
- **Both ends (symmetric).** The refusal holds on the number that is the operator's telephone and on the number that is H's own WhatsApp. Writing to H's WhatsApp would give it orders; writing from H's WhatsApp to the operator would pass for H to the person who gives it orders ([04](../04-seguranca-conexao.md), C4).
- **Code.** AR-705 for a chat or a destination, AR-712 for a message. The Hermes is not told why. No new code.
- **Where.** After the scope and line check, so a chat outside the scope keeps its answer. For a send by telephone, before the send-limit place is spent: no place spent, no engine call.
- **A chat the engine names only by a hidden identifier (`@lid`).** The engine keeps a table from hidden identifier to telephone (`referencia/waha-fatos.md`, last section). For a write in such a chat, on a number that has at least one recorded counterpart, the hub asks the engine for the telephone of that identifier and matches it like any telephone. If the engine does not answer a telephone, or fails, the write in that chat alone is refused (it fails closed). No access line is created, reading is never touched, no other chat is touched, and a number with no recorded counterpart makes no such call.
- **A Hermes removed:** its record goes in the same write, with an event. **Revoked, suspended or in quarantine:** the record stays and refuses.
- **The risk is said wherever the record is set.** The command `arauto-hub admin hermes whatsapp NAME` prints a warning while nothing is protected for that Hermes, shown or changed, and on a change that protects fewer chats. It never asks, because agents run it. `arauto-hub admin number list` ends with a line while no Hermes protects any chat, and tags each number `(gives orders to {hermes})` or `({hermes} takes orders here)`.
- **Audit.** `hermes.whatsapp_orders_changed` (security event: counts, never a telephone) and `whatsapp.order_chat_refused` (not a security event): the Hermes, the number, the verb, `matched` or `hidden`, never a telephone, at most one per Hermes, number and chat per hour, kept in the memory of the process (a restart forgets it, so at most one extra event per key).
- **The "command" mark of a number goes (part A0).** Migration 0020 drops `number.command`, with no carry-over. The flag `--command` of `number add`, the warning on the access page and in `access grant`, the box and the hint in the Add window, the tag on the list, the "(command channel)" suffix of `number list` and their strings in the nine languages are gone. Past `number.added` events keep the old flag.
- Migration 0021 holds the record: `hermes_whatsapp_phone` and `hermes_whatsapp_phone_digest`. One bridge telephone per Hermes, and one Hermes per bridge telephone in the organization.

It applies decisions [0004](0004-o-hub-so-isola.md) (the hub only isolates: it refuses one chat to everyone, as a chat line without "send" does, and looks at no content) and [0005](0005-comando-e-dado-sao-separados.md) (an order never comes through Arauto). Nothing in 0005 is reopened here. A dated note on 0005 saying what "outside Arauto" means when one end of the channel is a number of the hub **waits for the maintainer**; it is not written.

## Choices taken for the build

Marked proposal; the maintainer said to go on with the recommendations, and none is a decision of his in so many words.

- Symmetric refusal: yes.
- A removed Hermes ends its protection: yes.
- The refusal event, once an hour in memory: yes.
- Showing the risk when nothing is recorded, including a first empty record: yes (the command's warning and the closing line of `number list`; the window of the panel is part B, built).
- Hidden identifiers: settled by a trial of 17/11/2026 recorded in `referencia/waha-fatos.md`: the engine names one person both as `@c.us` and as `@lid`, and `GET /api/{session}/lids/{lid}` answers the hidden identifier with the telephone ending in `@c.us`. This replaces the two options of the design (let it pass, or refuse the hidden chat alone) with the lookup and a refusal only when it fails.
- `--orders-from` replaces the whole list; a lone `--takes-orders-on none` is refused while a list exists.
- The hold on a send by telephone is skipped for a Hermes without "send" on the number, so that it keeps its ordinary AR-701 and learns nothing about who is recorded.
- The telephone of the bridge cannot also be in the list of telephones that give orders to the same Hermes, in the command and in the panel (18/11/2026). Before, it was accepted and protected the self-chat pair while the mode said list.
- In the panel only, a typed telephone that does not start with "+" stops the save once with a read-back of each line; the command accepts it as before (18/11/2026).

## What is not guaranteed

- **Only what is recorded is refused.** A command channel the admin did not record is open to writing through Arauto. The hub cannot know an unrecorded one. That is why the command and `number list` say so.
- **The lookup of a hidden identifier was answered by a real engine once**, in a trial, and the call built here is covered by the fake engine only; the identifier is sent whole, percent-encoded, and that form is untried. If a real engine answers otherwise, a write in a chat shown by a hidden identifier is refused on a number that has a counterpart, and a Hermes reaches that person only by a telephone it learnt elsewhere.
- **Groups** are never protected. Whether a bridge takes orders in a group was not tried.
- **Owner forwarding and the open policy of the bridge** are not modelled. The mode and the telephones are typed by the admin; nothing reads them from Hermes.
- **Reading stays.** A Hermes that reads that chat sees the orders and the connection sentence, and can use an invite first; the real connection then fails and the hub raises an alert ([04](../04-seguranca-conexao.md), C4, which this decision does not change).
- **Only the Brazilian ninth digit** is matched across forms of a telephone.
- **A call already past the gate when a record is saved still runs;** the next call sees the record.
- **A restored database** keeps the records of the backup and loses the later ones; writing a record is refused until the restore is resolved.
- **The Hermes can infer** that a chat is protected, by the answer.
- **Ways around Arauto** (another linked device, a terminal tool of the Hermes, other platforms) are outside the hub.
- **Not tried** on a real Hermes, on a real WhatsApp, or against the real engine for the lookup of hidden identifiers. In the build, the full test binary of the panel did not finish when run whole (it was run by filter), and the migrations ran only on the test database.

## Alternatives considered

- **A mark on the number.** It cannot say whose orders or which chat. It was the old mark, and its name did not say the direction.
- **A free list of pairs of telephones.** Nothing ends it when a Hermes goes.
- **A control in the access editor.** A second control for one idea; the record belongs to the Hermes.
- **Refusing at grant time.** The same verbs serve every other chat, so nothing is refused when the access is given.
- **Letting a hidden chat pass,** or **refusing every hidden chat.** The lookup of the engine ends both.
- **A chat line for the hidden chat.** The existing rule gives every other hidden chat of an access with any chat line a line without verbs, reading included, so one line would close reading in all of them.
- **AR-701 as the answer.** Its sentence says only the admin can grant the verb, which is false here.

## Consequences

Dated notes in [03](../03-arquitetura.md), [05](../05-modelo-de-ameacas.md), [06](../06-modelo-de-dados.md), [07](../07-telas.md), [08](../08-permissoes.md), [09](../09-contrato-mcp.md), the [glossary](../glossario.md) and decisions [0039](0039-matriz-e-ferramentas-de-whatsapp-no-mvp.md) and [0044](0044-numeros-no-painel-sobre-o-motor.md). [04](../04-seguranca-conexao.md), the connection block of [02](../02-como-funciona.md) and [0005](0005-comando-e-dado-sao-separados.md) are **not touched**; each needs the maintainer. Part B, the panel, is built (17/11/2026) in `crates/arauto-hub/src/panel/orders.rs` and tested in `crates/arauto-hub/tests/panel/orders.rs` against a fake engine; it was **not tried in a browser**. It has the window of the Hermes tab (T1) with the fixed risk sentence, the window that asks first (a save that leaves no chat protected, or that unprotects one; case (c), a telephone reported by the connector, belongs to part E and does not exist), the tags on the numbers (T4) and, asked by the maintainer on 17/11/2026, a way to set the record from the numbers tab at any time after a number is in the list: the "Orders" action of a number leads, through a short form that chooses nothing beforehand, to the window of the chosen Hermes with the number already chosen, saving nothing by itself; in that window the numbers of the hub are chosen by alias (option B, 18/11/2026). The panel writes through the same admin function as the command (no second write path). The panel has no Hermes removal, so the removal line of the design is printed by `admin hermes remove` when the Hermes had a record. The warning at grant time (part C) and the connector report (part E) are not built.

Tests that fail when their rule is removed: the write in the chat refused for every Hermes (with the gate disabled, 10 of the 16 tests of the rule fail), the removal of a record with its Hermes, the migrations 0020 and 0021, the hidden identifier refused when the engine does not answer, the refusal event once an hour, the telephone of the bridge refused as a telephone that gives orders (command and panel), and a warned line saved only with a valid acknowledgement.
