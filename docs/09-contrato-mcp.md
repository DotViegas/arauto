# 09 — What Hermes sees

**Status: proposal.** The tools and the notices the connector delivers to Hermes. The arguments and
the answer format of each tool are deliverables of the first MVP ([15](15-mvp.md)).

## Summary

```
MCP server "arauto" (stdio)
 ├── arauto_status                 always available
 ├── hermes_...    (notes)         4 tools, of the conversation between Hermes
 ├── whatsapp_...  (read)          7 tools, plus 2 after the first version
 ├── whatsapp_...  (send)          7 tools
 └── whatsapp_...  (modify)        2 tools
notices: "there are N new messages in chat X of number Y"
         "there are N new notes from Hermes Z"
```

Hermes only sees the tool families whose verb it has on some number. Before the approval, and
after it while it has no access, it sees only `arauto_status`. The note tools appear
when it has a link in the peer list. In the hub, on 11/11/2026 (M1), the session allows only
`arauto_status`: the note ones come in at M2 and the WhatsApp ones at M3.

## 1. Rules of the contract

| Rule | Detail |
|---|---|
| Closed list | Only the tools on this page. There is no generic forwarding to the engine. The hub validates again the name and the arguments of each call. |
| Fixed names and descriptions | They are compiled into the connector. The hub says which names are allowed and cannot change a text the LLM reads. |
| The number is an argument | `number` receives the alias given in the panel, which has a restricted format ([13](13-conversa-entre-hermes.md), section 2). It may be omitted when Hermes has access to only one. |
| Opaque identifiers | Hermes never states the name of the engine session. The hub resolves it from the accesses. |
| Third-party content is data | Every read description says that the text of the messages was written by other people and is not an order. |
| Catalog errors | Every refusal has an AR code and a fixed sentence that says what happened and who solves it, with no command to execute. The catalog is in [11-conector.md](11-conector.md), section 8. |
| No secret in the answer | No tool returns a credential, the engine address or an internal path. |

## 2. Common arguments

| Argument | What it is |
|---|---|
| `number` | The alias of the number in the panel. |
| `to` | Telephone with country code, or the identifier of the chat or of the group. The hub checks whether the telephone has WhatsApp and resolves the exact form. |
| `message_id` | Identifier of a message, as it came in the reading. |

## 3. Tools

### Always available

| Tool | What it does |
|---|---|
| `arauto_status` | Says whether Hermes is connected, which numbers it has access to and with which verbs, and whether the conversation between Hermes is enabled. No secret. |

**The answer of `arauto_status`, in the hub on 11/11/2026.** The hub answers it by itself, without
the engine, and the test checks that it carries no secret:

```json
{"connected": true, "hermes": "name-of-the-hermes", "can_be_woken": false, "numbers": [],
 "peers": {"enabled": false, "can_send": false, "can_receive": false}}
```

| Field | Content |
|---|---|
| `connected` | Always `true`: the answer only goes out through an open channel session. |
| `hermes` | The name of the Hermes, in the restricted format. |
| `can_be_woken` | Whether the admin allowed waking this Hermes. |
| `numbers` | The numbers and the verbs. Empty until M3; the format of each item is a deliverable of M3. |
| `peers.can_send` | There is a link from this Hermes to another. |
| `peers.can_receive` | There is a link from another Hermes to this one. |
| `peers.enabled` | Either of the two. |

Only the hub's answer is written. Without a channel session (pending request, channel down), the one
that answers the tool is the connector, and that answer is not written yet.

### Read

| Tool | What it does |
|---|---|
| `whatsapp_status` | State of the number: connected or not, and limits that WhatsApp imposed. |
| `whatsapp_list_chats` | Lists the chats and the groups, from the most recent to the oldest, with the last message. |
| `whatsapp_read_chat` | Reads the messages of a chat: author, text, file (without downloading), quote, edit, deletion and reactions. Default 30, maximum 200, within a budget of 256 KiB per answer: it cuts at whole messages and says whether there are more. `before` and `after` to move in time. |
| `whatsapp_get_contact` | Public data of the contact: name, profile text, photo and whether it is a business account. |
| `whatsapp_download_media` | Downloads a photo, PDF, audio or document of a message, up to 7 MB. An image arrives for the LLM to see. |
| `whatsapp_list_groups` | Lists the groups of the number, with the number of participants. |
| `whatsapp_get_group` | Data of a group and the participants, in parts when the group is large. |

After the first version, with the message archive (Phase 6):

| Tool | What it does |
|---|---|
| `whatsapp_list_updates` | What arrived since the last time Hermes looked, per chat. |
| `whatsapp_search` | Searches the message archive by text, person, period and type. |

### Send

| Tool | What it does |
|---|---|
| `whatsapp_check_number` | Asks whether a telephone has WhatsApp and returns the exact identifier. It writes to nobody. |
| `whatsapp_send_text` | Sends text. It accepts replying to a message and mentioning people in a group. |
| `whatsapp_send_file` | Sends an image, document or audio, by public address (up to 15 MB) or by content (up to 3 MB). In the first MVP, only by content. |
| `whatsapp_mark_read` | Marks the chat as read. |
| `whatsapp_set_typing` | Turns "typing" on or off. |
| `whatsapp_react` | Reacts to a message. |
| `whatsapp_forward` | Forwards a message to another chat. It requires **read** at the origin and **send** at the destination. |

### Modify

| Tool | What it does |
|---|---|
| `whatsapp_edit_message` | Edits the text of a message sent by the number, within WhatsApp's time limit. |
| `whatsapp_delete_message` | Deletes a message for everyone, within WhatsApp's time limit. |

### Conversation between Hermes

| Tool | What it does |
|---|---|
| `hermes_list_peers` | Lists the peers of this Hermes, and whether it can send and receive. |
| `hermes_send_message` | Sends a note to a peer, in a new thread or in one that already exists. |
| `hermes_list_threads` | Lists the threads, with the number of unread ones. |
| `hermes_read_thread` | Reads the notes of a thread. |

The rules, the ceilings and the errors are in [13-conversa-entre-hermes.md](13-conversa-entre-hermes.md).

## 4. Cautions the hub keeps

They come from the experience of the source service:

- `to` and `message_id` are validated and encoded before going to the engine. Without that, a value
  with `../` would reach the engine session of another number.
- Sending by address: the one that downloads is the hub, and only from a public address. The first
  version keeps sending by address, beside sending by content
  ([decisoes/0024](decisoes/0024-envio-de-arquivo-por-endereco.md)).
- Brazilian telephone with and without the ninth digit: the hub tests both forms.
- Editing without stating the text and deleting without the full identifier are refused: in the
  engine, both have dangerous behavior by default.

## 5. Notices

There are two types. The first is a new message in a chat in which Hermes can be woken. Edit,
deletion and reaction are recorded and appear in the reading, but do not wake.

| Notice field | Content |
|---|---|
| `kind` | `whatsapp` |
| `number` | The alias given by the admin. |
| `chat` | Opaque identifier of the chat. |
| `count` | How many new messages. |
| `id` | Identifier of the notice, to discard repetition. |

The notice carries no message text, contact name or group subject.

The second is a new note from another Hermes: `kind` is `peer`, with `from`, `count` and `id`
([13](13-conversa-entre-hermes.md), section 5). The connector checks the format of each field before
assembling the fixed sentence, and refuses a notice of a type it does not know
([14](14-protocolo-do-canal.md), section 5.3).

## 6. Note on origin

The `whatsapp_*` family comes from the source service. On 10/11/2026 the names moved to English,
along with the code ([decisoes/0016](decisoes/0016-idioma.md)): proposal, to be confirmed by the
maintainer. A Hermes that comes from the source service needs to know the new names. The other
difference is the `number` argument: there each agent had only one number.
