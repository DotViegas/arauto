# 0039 — The access matrix and the WhatsApp tools in the first MVP

**Status:** proposal · **Date:** 12/11/2026, with the code of the matrix and of the tools in the
hub (M3, second part)

## Context

The [08-permissoes.md](../08-permissoes.md) gives the four verbs, the scope, the chat line and the
decision rule; the [09-contrato-mcp.md](../09-contrato-mcp.md) gives the list of the tools, but
leaves the arguments and the format of the answers to the MVP; the [06-modelo-de-dados.md](../06-modelo-de-dados.md),
section 5, gives the tables. This decision records what the code chose where the texts are silent,
and where it moved away from them. Everything was tested only against the fake engine.

## Decision

### The matrix

- **Tables** `access` (one per pair Hermes × number: `scope` `all`/`list`, `can_read`,
  `can_send`, `can_modify`, `can_wake`, `can_start_chats`, `created_by`, `created_at`,
  `changed_at`) and `access_chat` (the line: the four verbs per chat), in migration 0010. The
  database refuses "be woken" without "read", "may start chats" without "send" and scope "all", and
  a chat line of another number (composite foreign key with the chat and the number).
- **The verbs of the number** apply to what belongs to the number (state, listings, checking a
  phone) and, with scope "all", in every chat without a line. With scope "list", only the chats
  with a line exist. The line replaces entirely; a line without a verb blocks.
- **The tool family** of a verb is released when Hermes has that verb on some number, in the line
  of some chat included. The listings (`whatsapp_list_chats`, `whatsapp_list_groups`) and
  `whatsapp_status` require "read" somewhere in the number and show only the chats Hermes may
  read; `whatsapp_check_number` requires "send" on the number itself.
- **A chat that "does not exist yet"** (rule 5): without a message archive, it is the chat the hub
  does not have and in which the engine has no message. It requires scope "all", "send" and "may
  start chats"; otherwise AR-701 (with "all") or AR-705 (with "list").
- **Number swapped for the accesses**: it releases no tool at all, and every call that names it
  receives AR-702, until `admin number confirm` or `unpair`.
- **End of the accesses**: removing the number or the Hermes erases its accesses in the same
  write, each one with its event.
- **Audit**: `access.granted`, `access.changed`, `access.removed`, `access_chat.set` and
  `access_chat.removed`, all security events, with the before and the after (scope, verbs and
  "may start"). Written by `begin_security`: a restored database receives nothing.
- **Command**: `arauto-hub admin access grant HERMES NUMBER --scope all|list [--read] [--send]
  [--modify] [--wake] [--start-chats]` (the whole access; the scope is mandatory),
  `access remove`, `access chat HERMES NUMBER CHAT [verbs]`, `access chat-remove` and
  `access list [HERMES]`. The chat is a phone (the engine gives the exact form, with or without the
  ninth digit; without the engine up, it refuses), an identifier of the engine (`...@g.us`,
  `...@c.us`, `...@lid`) or the identifier of the hub shown by `access list`. The command warns,
  and does not prevent, when it gives "send" on a number marked as a command number and when
  another Hermes also sends or is woken on the same number (08, sections 5 and 6).

### The list of tools

- The `session` carries what the matrix releases now. At each change, the session sends `tools`:
  it is nudged by the notice of the command (`hermes:{id}`, also for each Hermes of a removed
  number) or by the sweep, which compares, in one query per organization, what each session
  received with what Hermes has. A `number:{id}` asks for the sweep right away.
- **No call trusts the list**: each WhatsApp call reads the access in the database, in a query by
  index, and the line of the chat when it needs to.

### Identifiers

- **`number`** is the alias. Omitted, the only number of the Hermes applies; with more than one,
  AR-711. The alias of a number without access and a nonexistent alias give the same answer
  (AR-701).
- **`chat`** is the UUID of the `chat` row of the hub. The listing creates the row of the chat the
  hub does not have yet (only with scope "all": with "list", a chat without a line does not appear).
- **`to`** is a phone (with the country code; the hub asks the engine for the exact form) or a chat
  UUID. An identifier of the engine (`...@c.us`) is not accepted.
- **`message_id`** is signed by the hub: base64url of `chat (16 bytes) | HMAC-SHA-256 (32) |
  identifier in the engine`, with the key of the WhatsApp digests of the organization. It is not
  encrypted: it carries the chat and the author, which the read already shows; it is the same at
  each read. Wrong signature, of another organization, of a chat of another number or outside the
  scope: AR-712. The message names its chat, and the hub checks that it is the signed chat.

### The send ceiling

- 20 in any window of 60 seconds, per Hermes and per number, on the clock of the hub. They count:
  `whatsapp_send_text`, `whatsapp_send_file`, `whatsapp_react`, `whatsapp_forward` and
  `whatsapp_check_number` (which asks WhatsApp about outside people); marking as read and
  "typing" do not count. The call that passed all the checks of the hub and goes to the engine
  counts, even if the engine fails; a refusal by the hub does not count.
- The refusal is AR-704 with `retry_after_s` (when the oldest leaves the window). The sentence of
  AR-704 with a wait entered the catalog, like that of AR-706.
- The count lives in the memory of the hub, under a lock: of many calls at the same time, exactly
  those that fit pass; another session of the same Hermes finds the same count. Restarting the hub
  zeroes the count; a second hub of the same organization is not handled.

### New code in the catalog

- **AR-712**: "Message not found: it does not exist, is outside what this Hermes can use, or
  WhatsApp no longer allows it." Used for a message identifier that is not valid and for a message
  that the engine does not find or no longer lets change.

### The tools and the answers

| Tool | Arguments | Answer |
|---|---|---|
| `whatsapp_status` | `number?` | `number`, `state`, `connected`, `whatsapp_limits` (`message_capping`, `reachout_timelock`) or `null` |
| `whatsapp_list_chats` | `number?`, `limit` (1–100, 20), `offset` | `number`, `chats` [`chat`, `kind`, `name`, `phone`, `last_message`], `more`, `next_offset` |
| `whatsapp_read_chat` | `number?`, `chat`, `limit` (1–200, 30), `before`, `after` (seconds) | `number`, `chat`, `messages` (the newest first), `more`, `next_before` |
| message (in the read) | | `message_id`, `time` (seconds), `from_me`, `author` (`+phone` or `null`), `text`, `file` (`mimetype`, `filename`) or `null`, `reply_to` |
| `whatsapp_get_contact` | `number?`, `chat` (of a contact) | `chat`, `phone`, `name`, `push_name`, `about`, `picture_url`, `is_business` (`null`) |
| `whatsapp_download_media` | `number?`, `message_id` | `mimetype`, `filename`, `size`, `data` (base64), up to 7 MB |
| `whatsapp_list_groups` | `number?`, `limit`, `offset` | `groups` [`chat`, `subject`, `participants`], `more`, `next_offset` |
| `whatsapp_get_group` | `number?`, `chat` (of a group), `limit` (1–200, 100), `offset` | `chat`, `subject`, `description`, `total`, `participants` [`phone`, `role`], `next_offset` |
| `whatsapp_check_number` | `number?`, `phone` | `phone`, `has_whatsapp`, `exact` |
| `whatsapp_send_text` | `number?`, `to`, `text` (up to 64 KiB), `reply_to?`, `mentions?` (up to 50 phones) | `chat`, `message_id` |
| `whatsapp_send_file` | `number?`, `to`, `kind` (`image`/`document`/`voice`), `mimetype`, `filename?`, `data` (base64, up to 3 MB), `caption?` | `chat`, `message_id` |
| `whatsapp_mark_read` | `number?`, `chat` | `done` |
| `whatsapp_set_typing` | `number?`, `chat`, `on` | `done` |
| `whatsapp_react` | `number?`, `message_id`, `reaction` (empty removes) | `done` |
| `whatsapp_forward` | `number?`, `message_id`, `to` | `chat`, `message_id` |
| `whatsapp_edit_message` | `number?`, `message_id`, `text` (mandatory) | `done` |
| `whatsapp_delete_message` | `number?`, `message_id` | `done` |

Every answer carries `number`. The read cuts at whole messages within 256 KiB; when there are
still messages, the page ends at a change of second, so that `next_before` (exclusive) does not
lose any. Editing and deleting require the complete identifier, which only comes from a message
read in the complete format; an identifier of WhatsApp only is refused with AR-711.

**`arauto_status`**, each item of `numbers`: `number` (alias), `state`, `scope`, `verbs` (those of
the number), `chat_verbs` (those that some chat line gives) and `can_start_chats`. Nothing of the
phone or of the name of the engine session. A removed number does not appear; a number swapped
appears with the state.

## Consequences

- The `docs/11-conector.md`, section 8, needs the line of AR-712, and the
  `docs/09-contrato-mcp.md` the format above; they are left for the merge with the connector line.
- Without a message archive, edits, deletions and reactions do not appear in the read (the engine
  does not bring them in the message listing), and every read depends on the engine being up.
- A read of 200 long messages may go over the ceiling of 4 MiB of the engine's answer and become
  AR-703.
- Alternatives considered: an encrypted message identifier (opaque in fact, but different at each
  read, which prevents Hermes from recognizing what it already read); the raw identifier of the
  engine (Hermes could write one of another chat); a ceiling in the database (it survives the
  restart, but puts a write in each send).
