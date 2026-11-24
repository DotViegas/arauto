# 0051 — The chat where a Hermes takes orders on WhatsApp is refused for writing

**Status:** proposal · **Date:** 17/11/2026. Asked by the maintainer. **Built on 17/11/2026** in three parts, A0, A1 and B, and covered by automatic tests against a fake engine; **not tried on a real Hermes or on a real WhatsApp, and part B was not tried in a browser**, the window of part B was rebuilt as option B on 18/11/2026 (the numbers of the hub chosen by alias, chosen by the maintainer), also tested against a fake engine only and **not tried in a browser**, and the texts were not approved by him word for word. Part B (the panel) is built and tested against a fake engine only; parts C and later (the warning at grant time, the connector report) **do not exist**; they are not described here as built. R1 and R2 of 18/11/2026 (below) are built and covered by automatic tests against a fake engine and a fake Hermes, not tried on a real Hermes. R3 of 18/11/2026 (orders from outside WhatsApp, below) is built and covered by automatic tests against a fake engine, **not tried in a browser or on a real Hermes**; its texts were not approved by the maintainer word for word.

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
- **The risk is said wherever the record is set.** The command `arauto-hub admin hermes whatsapp NAME` prints a warning while nothing is protected for that Hermes, shown or changed, and on a change that protects fewer chats. It never asks, because agents run it. `arauto-hub admin number list` ends with a line while no Hermes protects any chat, and tags each number `(gives orders to {hermes})` or `({hermes} takes orders here)`. (18/11/2026, R3: a Hermes recorded as taking its orders outside WhatsApp counts as recorded: no warning, and the closing line goes once any Hermes has an answer; see the section on R3.)
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
- One code for chat and message calls (18/11/2026): AR-714 for both, since the Hermes needs the same instruction in either case.
- AR-705 by telephone under the scope "list", also to the owner (18/11/2026): the ordinary answer of that scope comes first.
- The sentences are proposals, not approved word for word (18/11/2026).
- In the panel only, a typed telephone that does not start with "+" stops the save once with a read-back of each line; the command accepts it as before (18/11/2026).

## 18/11/2026: the Hermes whose command channel it is is told so (R1, R2)

**Field report (production, 18/11/2026, one occurrence).** A Hermes whose own WhatsApp is a number of the hub tried three times to send a notice, through Arauto, to a telephone that gives it orders. It got AR-705 each time, told its operator that Arauto did not let it send, and stopped. Told to use its own WhatsApp, it delivered at the first try. The maintainer asked that this be implicit: nobody should have to tell the Hermes.

The maintainer approved three changes; two are here (the third, for a Hermes that takes orders outside WhatsApp, is the next section).

- **R1: a distinct answer for the Hermes whose chat it is.** When the caller is a Hermes whose **own** record makes the chat protected, the write is refused with **AR-714** in place of AR-705 or AR-712. Same places, same engine calls (none), same effect on the send limit (none). The connector turns the code into a fixed sentence, compiled into it (principle 6: the hub sends no free text): "This chat is your command channel, as the admin recorded it, so Arauto does not write in it. Reply or report in it through your own WhatsApp, outside Arauto." One code serves a chat, a destination and a message. It holds on both ends of the pair, in self-chat, for a hidden identifier the engine translated to a recorded telephone, and for any form of a telephone. A chat in the records of two Hermes gives AR-714 to each of them. **Every other Hermes keeps AR-705 or AR-712 and is told nothing.** This replaces "No new code" for that one Hermes; "The Hermes is not told why" still holds for every other.
- **Where AR-714 is not said.** A hidden identifier the engine did not translate (the hub cannot tell whose chat it is): AR-705 or AR-712 as before. An ordinary answer that comes first: AR-701 without "send", AR-705 outside the scope or the line; a sending by telephone under the scope "list" answers AR-705 to every Hermes, the owner included. A connector of protocol version 1.
- **Protocol version 2.** An installed connector of version 1 treats an unknown code as an invalid `result` and drops its channel, so the hub says AR-714 only on a connection where version 2 was agreed. Version 2 is version 1 plus that one code. Both programs speak versions 1 to 2, so a connector installed before keeps working. The new code reaches a Hermes only after it is reconnected, because the hub never updates a running connector ([14](../14-protocolo-do-canal.md), section 5.1; [11](../11-conector.md), section 7).
- **R2: say it up front.** The descriptions of `whatsapp_send_text`, `whatsapp_send_file` and `whatsapp_forward` end with: "Do not write through Arauto in a chat where you take orders: reply or report in that chat the way its orders reach you, outside Arauto." The text is compiled into the connector. No MCP server instructions were added, because it is unchecked whether Hermes passes them to the model.
- **Audit.** `whatsapp.order_chat_refused` gains `own_chat` (true or false) and `code` (the code answered). Never a telephone. The key (Hermes, number, chat) and the once-an-hour rule are unchanged.

## 18/11/2026: orders from outside WhatsApp (R3)

A Hermes may take its orders on another conversation platform, like Telegram. Arauto cannot write there, so nothing needs refusing for it, and sending to WhatsApp numbers through Arauto is fine. Until now such a Hermes could only be left "not recorded", which raised false alarms. R3 adds an explicit answer, recorded by the admin: **its orders come from outside WhatsApp.** It answers "who gives this Hermes orders on WhatsApp" with "nobody": no telephone, nothing refused, the gate untouched. It counts as recorded. Arauto writes to no other platform.

- **Where it is set.** The command flag `--orders-outside-whatsapp` (`arauto-hub admin hermes whatsapp NAME --orders-outside-whatsapp`; it clears the recorded telephones, and the command refuses it together with `--self-chat` or `--orders-from`), and the panel option "Outside WhatsApp: another conversation platform, like Telegram" under "Orders come from", with the hint "Arauto then refuses nothing for {hermes}. The hub cannot check it: if {hermes} also takes orders on a WhatsApp, record that WhatsApp instead."
- **Each answer replaces the other.** It cannot be recorded together with a telephone of the Hermes's WhatsApp, self-chat or a list: the record is refused (the command and the panel, with a sentence of its own), and nothing is written. Recording a telephone, a list or self-chat after it replaces it; recording it after them clears the telephones. `--takes-orders-on none` and an empty list are accepted with it.
- **What it silences.** The warning of the command ("nothing is recorded"), the muted "not recorded" of the list column (it says "outside WhatsApp", not muted), the risk window of the panel on a save from nothing, and the closing line of `number list`. In their place the command prints: "Note: the hub cannot check this. If {hermes} also takes orders on a WhatsApp, record that WhatsApp instead, with --takes-orders-on and --self-chat or --orders-from."
- **What still asks.** A save from a protected list to "outside WhatsApp" unprotects chats, so it still shows the window that asks and names those chats, and the command still prints the line for fewer chats.
- **Storage.** Migration 0023, a table of its own, `hermes_orders_outside_whatsapp` (one row per Hermes, removed with it). The gate (the counterparts), the tags and the check of one bridge per telephone never read it. The audit event of the change carries the word `outside_whatsapp` in `mode_before` and `mode_after`, with counts and words only; removing the Hermes writes the event with it. `arauto-hub admin hermes remove` now prints its removal line only when a chat was protected (before, it also printed it for a lone bridge telephone, which protected nothing).

## What is not guaranteed

- **Orders from outside WhatsApp: the hub cannot check it.** A wrong answer leaves the real WhatsApp command chat as open as an unrecorded one, while silencing the warnings. A Hermes that also takes orders on a WhatsApp must have that WhatsApp recorded instead: the two cannot be recorded together. The closing line of `number list` goes once any Hermes has an answer, even while another has nothing (as before). Nothing contradicts a wrong answer: no check compares the answer with what the engine shows.
- **Only what is recorded is refused.** A command channel the admin did not record is open to writing through Arauto. The hub cannot know an unrecorded one. That is why the command and `number list` say so.
- **The lookup of a hidden identifier was answered by a real engine once**, in a trial, and the call built here is covered by the fake engine only; the identifier is sent whole, percent-encoded, and that form is untried. If a real engine answers otherwise, a write in a chat shown by a hidden identifier is refused on a number that has a counterpart, and a Hermes reaches that person only by a telephone it learnt elsewhere.
- **Groups** are never protected. Whether a bridge takes orders in a group was not tried.
- **Owner forwarding and the open policy of the bridge** are not modelled. The mode and the telephones are typed by the admin; nothing reads them from Hermes.
- **Reading stays.** A Hermes that reads that chat sees the orders and the connection sentence, and can use an invite first; the real connection then fails and the hub raises an alert ([04](../04-seguranca-conexao.md), C4, which this decision does not change).
- **Only the Brazilian ninth digit** is matched across forms of a telephone.
- **A call already past the gate when a record is saved still runs;** the next call sees the record.
- **A restored database** keeps the records of the backup and loses the later ones; writing a record is refused until the restore is resolved.
- **The Hermes can infer** that a chat is protected, by the answer. Since 18/11/2026 the Hermes whose command channel it is learns it from AR-714, which is its own record; another Hermes learns nothing more than before. A Hermes can learn by sending whether a telephone is in its own record, at no cost in the send limit (as AR-705 already allowed).
- **An LLM may still write there first:** the sentences tell, they do not make the Hermes obey; not tried on a real Hermes.
- **Mixed copies of the connector on one machine:** a newer copy that leads older copies passes AR-714 on to them, and an older copy answers AR-207 for that call.
- **Until a Hermes is reconnected,** it keeps AR-705 or AR-712 and the old descriptions.
- **Ways around Arauto** (another linked device, a terminal tool of the Hermes, other platforms) are outside the hub.
- **Not tried** on a real Hermes, on a real WhatsApp, or against the real engine for the lookup of hidden identifiers. In the build, the full test binary of the panel did not finish when run whole (it was run by filter), and the migrations ran only on the test database.

## Alternatives considered

- **A mark on the number.** It cannot say whose orders or which chat. It was the old mark, and its name did not say the direction.
- **A free list of pairs of telephones.** Nothing ends it when a Hermes goes.
- **A control in the access editor.** A second control for one idea; the record belongs to the Hermes.
- **Refusing at grant time.** The same verbs serve every other chat, so nothing is refused when the access is given.
- **Letting a hidden chat pass,** or **refusing every hidden chat.** The lookup of the engine ends both.
- **A chat line for the hidden chat.** The existing rule gives every other hidden chat of an access with any chat line a line without verbs, reading included, so one line would close reading in all of them.
- **Sending AR-714 without a version.** An installed connector would drop its channel and answer AR-207.
- **An optional field on `result` instead of a version.** It breaks: only the code travels.
- **Server instructions in `initialize` as the only carrier.** Unchecked whether Hermes passes them to the model.
- **A list of command chats pushed to the connector at connect time.** State in the connector, and one saved call.
- **AR-701 as the answer.** Its sentence says only the admin can grant the verb, which is false here.

## Consequences

Migration 0023 (R3, 18/11/2026) is covered by a test of the migration and by tests of the command, the record and the panel; the security rule that outside never coexists with a telephone has tests that fail when the check is removed. Docs touched by R3: [06](../06-modelo-de-dados.md), [07](../07-telas.md), [08](../08-permissoes.md) and the [glossary](../glossario.md).

Dated notes in [03](../03-arquitetura.md), [05](../05-modelo-de-ameacas.md), [06](../06-modelo-de-dados.md), [07](../07-telas.md), [08](../08-permissoes.md), [09](../09-contrato-mcp.md), the [glossary](../glossario.md) and decisions [0039](0039-matriz-e-ferramentas-de-whatsapp-no-mvp.md) and [0044](0044-numeros-no-painel-sobre-o-motor.md). [04](../04-seguranca-conexao.md), the connection block of [02](../02-como-funciona.md) and [0005](0005-comando-e-dado-sao-separados.md) are **not touched**; each needs the maintainer. Part B, the panel, is built (17/11/2026) in `crates/arauto-hub/src/panel/orders.rs` and tested in `crates/arauto-hub/tests/panel/orders.rs` against a fake engine; it was **not tried in a browser**. It has the window of the Hermes tab (T1) with the fixed risk sentence, the window that asks first (a save that leaves no chat protected, or that unprotects one; case (c), a telephone reported by the connector, belongs to part E and does not exist), the tags on the numbers (T4) and, asked by the maintainer on 17/11/2026, a way to set the record from the numbers tab at any time after a number is in the list: the "Orders" action of a number leads, through a short form that chooses nothing beforehand, to the window of the chosen Hermes with the number already chosen, saving nothing by itself; in that window the numbers of the hub are chosen by alias (option B, 18/11/2026). The panel writes through the same admin function as the command (no second write path). The panel has no Hermes removal, so the removal line of the design is printed by `admin hermes remove` when the Hermes had a record. The warning at grant time (part C) and the connector report (part E) are not built.

Tests that fail when their rule is removed: the write in the chat refused for every Hermes (with the gate disabled, 10 of the 16 tests of the rule fail), the removal of a record with its Hermes, the migrations 0020 and 0021, the hidden identifier refused when the engine does not answer, the refusal event once an hour, the telephone of the bridge refused as a telephone that gives orders (command and panel), and a warned line saved only with a valid acknowledgement.

R1 and R2 (18/11/2026) add tests: the ownership test fails when the check of whose record it is is removed or inverted, and the version-1 test fails when the version gate is removed. Docs touched: [08](../08-permissoes.md), [09](../09-contrato-mcp.md), [11](../11-conector.md), [14](../14-protocolo-do-canal.md), the [glossary](../glossario.md) and `referencia/hermes-fatos.md` (N24).

## Note of 24/11/2026 (decision 0054)

Changed at the maintainer's request of 24/11/2026 ("Começe implementando tudo. Você pode decidir como implementar para que tudo funcione."). Wording by his assistant, not reviewed word for word by him.

For the Hermes that takes orders in a chat, the admin keeps "be woken" off in that chat, with a line of the chat ([0049](0049-acordar-pelo-whatsapp-so-pelo-verbo.md) already allows it). Reason: the bridge of that Hermes already delivers those messages as orders, and a wake by Arauto would start a second run that reads them as data, in the Hermes session of the contact ([0054](0054-uma-sessao-do-hermes-por-contato.md), open choice C7, answered as the recommended column: no rule of the hub). For every other Hermes, that chat is a contact chat like any other. The warning of part C, in the panel when "be woken" is turned on there, is **still not built**. Nothing else in this decision is reopened.
