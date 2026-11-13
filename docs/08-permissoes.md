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
- **Be woken** requires **read** on the same line.
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

The change applies on the next call. A notice that is already in the queue is evaluated again on
delivery and discarded if the access was removed.

### What Hermes sees in each case

| Situation | Answer |
|---|---|
| Device in quarantine, revoked or suspended; incompatible connector | AR-501, AR-502, AR-506 or AR-401 |
| There is no access to the number, or the verb is missing on the number | AR-701 |
| The chat is outside the scope, or the line of the chat does not have the verb | AR-705, the same answer as for a chat that does not exist |
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

The panel shows, per Hermes, whether it can be woken. When it cannot, the verb stays disabled, with
the reason. While the key of Hermes's local API is not resolved
([11-conector.md](11-conector.md), section 6), no Hermes can.

## 5. What the hub does not decide

- Filtering by content, word, time of day or type of contact.
- Choosing which Hermes answers.
- Answering by itself.
- Holding a message for approval.

When two Hermes can send or be woken in the same chats, the panel **warns and does not
prevent**.

## 6. Two cautions that are not permission

**The tools appear on all of Hermes's conversation platforms.** Whoever manages to give orders to
Hermes, through any conversation platform of its own, manages to use the granted numbers. Restricting
tools by conversation platform is configuration of Hermes, not of the hub. The panel says so on the
card of each Hermes.

**A number that is also a command channel.** If a number paired in the engine is in the list of those
who give orders to some Hermes, a Hermes with **send** on that number manages to write in the command
channel of the other. The hub does not know these lists. The panel allows marking a number as
"command" and alerts when **send** is granted on it.

## 7. Send ceiling

The hub has a technical send ceiling per Hermes and per number, so that a loop of the LLM does not
lead to the blocking of the number ([decisoes/0020](decisoes/0020-teto-de-envio.md)). The value is
fixed in the first version: 20 sends per minute, as a starting point. Above it, the send is refused
with AR-704. It is isolation, like protecting one neighbor from another: the hub does not look at the
content or at the recipient.
