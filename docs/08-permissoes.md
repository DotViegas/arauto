# 08 — Permissions

**Status: proposal.** What a Hermes may do on each number.

## Summary

```
R  READ           see chats and messages, search, download a file
S  SEND           write, react, mark as read and whatever else creates new activity
M  MODIFY         edit and delete what already exists
W  BE WOKEN       the hub notifies Hermes when a message arrives
```

Three rules always apply:

1. **Connecting is not permitting.** Every Hermes is born with access to no number.
2. **Absence is denial.** What was not marked does not exist for Hermes.
3. **The hub is the one that decides**, on every call and on every notice. The connector decides
   nothing.

## 1. The four verbs

| Verb | Allows | Does not allow |
|---|---|---|
| **Read** | Listing chats, reading messages, searching, downloading a file, seeing group participants, seeing the public data of the contact, seeing the state of the number. | Marking as read. |
| **Send** | Text, file, reply, reaction, "typing", marking as read, forwarding and checking whether a telephone has WhatsApp. | Starting a chat with someone who never talked to the number, except with the option "may start chats". |
| **Modify** | Editing and deleting a message. | Profile, privacy and blocks of the account. |
| **Be woken** | The hub notifies Hermes when a message arrives in a chat of the scope. | The content of the message inside the notice. |

- Checking whether a telephone has WhatsApp stays in **send** because it asks WhatsApp about someone
  from outside the chats, and only makes sense for whoever is going to write.
- **Be woken** requires **read** on the same line. It is the only setting that decides whether a
  WhatsApp message may wake the Hermes; there is no switch of the Hermes besides it (16/11/2026,
  [decision 0049](decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md)).
- **May start chats** requires **send** and scope "all".
- There is no verb that lets a Hermes create, pair, restart, disconnect or remove a number. That
  belongs only to the admin, in the panel.

## 2. Scope and chat line

| Scope | Meaning |
|---|---|
| **All chats** | The verbs apply to every chat of the number, including the ones that come up. |
| **Only chosen chats** | Only the listed chats exist for that Hermes. |

A **chat line**, when it exists, replaces entirely the verbs of the number in that chat.
A chat line with the four verbs off is equivalent to blocking.

To really isolate, prefer "only chosen chats". An allow list fails closed:
what is not in it does not exist. A restrictive chat line under "all" fails open if the
identifier of the chat changes in the engine.

## 3. Decision rule

Applied in the hub, in this order:

1. The device of the Hermes is not active (pending, approved, suspended, in quarantine, expired,
   revoked) or the connector is incompatible: **deny**.
2. There is no access between the Hermes and the number: **deny**.
3. There is a line for the chat: its verbs apply, and only they.
4. There is no line for the chat: with scope "all", the verbs of the number apply; with scope "list",
   **deny**.
5. Sending to a chat that does not exist yet: requires scope "all", **send** and "may start
   chats".
6. The call depends on the engine (send, modify, check a telephone, download a file that is not
   stored) and the number is unavailable or the engine is down: **error**. Reading what is already
   in the message archive does not depend on the engine.
7. The call writes (send or modify) in the chat where a Hermes takes orders on WhatsApp, as the
   admin recorded it: **deny**, for every Hermes, the one that takes the orders included. It comes
   after the scope and line check of rules 3 and 4, so a chat outside the scope keeps its answer.
   (17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md), proposal, built and tested against a fake engine; not tried on a real
   Hermes.) What is protected is a chat: the one between the telephone a Hermes's own WhatsApp runs
   on and a telephone that gives it orders or, in `self-chat` mode, the chat of that account with
   itself. It applies when one end is a number of the hub, on the number of either end. Refused:
   `send_text`, `send_file` and `forward` into it (by hub identifier or by telephone, in both
   Brazilian forms, existing or not; for a telephone before the send-limit place is spent),
   `mark_read`, `set_typing`, and `react`, `edit_message`, `delete_message` on a message of it. Not
   refused: reading, `check_number`, `forward` from it, being woken, every other chat of the number.
   A chat the engine names only by a hidden identifier (`@lid`), on a number that has a recorded
   counterpart: the hub asks the engine for the telephone of that identifier and matches it; if the
   engine does not answer, the write in that chat alone is refused. No access line is created and
   nothing is granted or refused at grant time. Only what the admin recorded is refused; see
   the cautions in section 6.

The change applies on the next call. A notice that is already in the queue is evaluated again on
delivery and discarded if the access was removed.

### What Hermes sees in each case

| Situation | Answer |
|---|---|
| Device in quarantine, revoked or suspended; incompatible connector | AR-501, AR-502, AR-506 or AR-401 |
| There is no access to the number, or the verb is missing on the number | AR-701 |
| The chat is outside the scope, or the line of the chat does not have the verb | AR-705, the same answer as for a chat that does not exist |
| A write in a chat, or to a telephone, where a Hermes takes orders on WhatsApp (rule 7) | AR-705; AR-712 for a message of that chat. Hermes is not told why: the answer is the one of a chat that does not exist or of a message that cannot be changed (17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md)) |
| Number disconnected or in "number swapped", on a call that depends on the engine | AR-702 |
| Engine down, on a call that depends on the engine | AR-703 |
| Send ceiling reached | AR-704 |
| In the conversation between Hermes: note ceiling reached; no peer with that name; thread not found | AR-706, AR-707, AR-708 ([13](13-conversa-entre-hermes.md)) |

Hermes gets to know that it does not have a verb on a number. It does not get to know that there is
a chat outside its scope.

## 4. Being woken

The notice is a pointer, with a fixed sentence ([03-arquitetura.md](03-arquitetura.md), section 7).
What Hermes does on waking is defined by the instructions of Hermes itself, not by the hub.

| Delivery rule | Value |
|---|---|
| Guarantee | At least once, with confirmation from the connector. A repeated notice is discarded by the connector. |
| Queue while Hermes is away | 24 hours or 1,000 notices. Past 1,000, the oldest expires. |
| Grouping | One pending notice per chat: what arrives later adds to the quantity. After a delivered notice, the next one of the same chat only leaves in 10 seconds. |
| Connector ceiling | 30 wakes per minute and 300 per hour, adding up the types. Above that the connector refuses, and the hub tries again in 5 minutes. |
| Message from the number itself | It wakes nobody. It avoids a loop. |

The notice of a note between Hermes follows the same rules, with the ceilings of
[13-conversa-entre-hermes.md](13-conversa-entre-hermes.md), section 6.

The wake mark of the link is enough ([decision 0045](decisoes/0045-a-marca-da-ligacao-basta-para-acordar.md), 15/11/2026). The follow mark (proposal, [decision 0050](decisoes/0050-etapa-b-marca-de-seguir.md), built, not tried on a real Hermes) is read when the notice leaves and only changes which fixed sentence the connector shows; the access checks on every call are unchanged.

A WhatsApp message wakes a Hermes when the verb "be woken" holds for the chat: on the access of the
number, or on the line of the chat when the chat has its own rule. Nothing else about the Hermes is
consulted; the other conditions stay (the kind of chat, the state of the number, an active device, the
notice limits). There is no switch of the Hermes and no command `arauto-hub admin wake`: to stop
waking by WhatsApp, turn "be woken" off on each access of the Hermes, or revoke it (16/11/2026,
[decision 0049](decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md); it replaces the switch "Woken by
WhatsApp" of decision 0045). In the access editor, turning the verb off also turns it off in the chats of
that access that have their own rule. A waiting WhatsApp notice is discarded when the verb or the access
is gone, or the number is not connected. Without the key of Hermes's local API
([11-conector.md](11-conector.md), section 6) no notice of either kind wakes it.

## 5. What the hub does not decide

- Filtering by content, word, time of day or type of contact.
- Choosing which Hermes answers.
- Answering by itself.
- Holding a message for approval.
- Giving any weight to the level of a Hermes or to the kind of a link: they decide nothing in the hub ([decision 0043](decisoes/0043-organograma-e-niveis.md), stage A, proposal). The level decides only where the admin may set the follow mark, at publish; at the time of a note the hub reads only the mark ([decision 0050](decisoes/0050-etapa-b-marca-de-seguir.md), proposal, built, not tried on a real Hermes). The mark gives the sender nothing in the hub: no access, no link, no number.

When two Hermes can send or be woken in the same chats, the panel **warns and does not
prevent**.

One exception, asked by the maintainer (17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md), proposal; the text was not approved word for word):
the hub **prevents** every Hermes from writing in the chat where a Hermes takes orders on WhatsApp, when
one end of it is a number of the hub and the admin recorded it. It applies decision 0005: an order never
comes through Arauto. Reading that chat follows the accesses.

## 6. Two cautions that are not permission

**The tools appear on all of Hermes's conversation platforms.** Whoever manages to give orders to
Hermes, through any conversation platform of its own, manages to use the granted numbers. Restricting
tools by conversation platform is configuration of Hermes, not of the hub. The panel says so on the
card of each Hermes.

**A number that is also a command channel.** If a number paired in the engine is in the list of those
who give orders to some Hermes, a Hermes with **send** on that number manages to write in the command
channel of the other. The hub does not know these lists on its own.

(17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md), proposal.) The "command" mark of a number and its alert are gone: the mark did
not say whether a number gives orders or takes them. The admin records, per Hermes, the telephone its
own WhatsApp runs on and who gives it orders there, with `arauto-hub admin hermes whatsapp NAME`;
the hub then refuses every Hermes writing in that chat (section 3, rule 7). Reading it follows the
accesses. **Not guaranteed:** a command channel nobody recorded is open to writing; the hub cannot
know it. While nothing is recorded for a Hermes, the command prints a warning, and `number list`
ends with a line while no Hermes protects any chat. The panel shows and sets it since part B (built and tested against a fake engine, **not tried in a browser**): the line action "Who gives it orders" of the Hermes tab opens a window with the same record, where the numbers of the hub are chosen by alias and only other telephones are typed (option B, 18/11/2026); the fixed risk sentence is always in it, and a save that leaves nothing protected or unprotects a chat asks first, names those chats, and saves nothing before the answer (screens T1 and T4, [07](07-telas.md)). The number tags and the "Orders" action of the numbers tab lead to the same window. What the panel writes is what the command writes, with the same audit. The telephone a Hermes's own WhatsApp runs on cannot also be in its list of telephones that give it orders, in the command and in the panel (18/11/2026): that chat is the self-chat mode. The risk window for a telephone a connector reported (part E) does not exist.

## 7. Send ceiling

The hub has a technical send ceiling per Hermes and per number, so that a loop of the LLM does not
lead to the blocking of the number ([decisoes/0020](decisoes/0020-teto-de-envio.md)). The value is
fixed in the first version: 20 sends per minute, as a starting point. Above it, the send is refused
with AR-704. It is isolation, like protecting one neighbor from another: the hub does not look at the
content or at the recipient.
