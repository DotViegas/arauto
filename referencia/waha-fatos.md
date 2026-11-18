# Facts checked on the engine (WAHA)

**Status: checked facts.** What Arauto assumes about WAHA, the program used as the engine.

**Checked on 09/11/2026, on the image `devlikeapro/waha:gows-2026.10.1`, with the GOWS internal mechanism, in the
CORE edition.** The cost and the license of WAHA are a matter for its author; this file does not replace
its documentation.

Marking: **[code]** read in the installed program; **[observed]** seen running;
**[documentation]** read in the author's public documentation, without running the program (the
section "The formats the hub uses").

## Warning

WAHA is an **unofficial** WhatsApp client. WhatsApp may block numbers that use it. Arauto does not
embed WAHA: whoever installs brings their own.

**License:** the `package.json` of the checked image declares `UNLICENSED`. Before any joint
distribution, check the terms published by the author.

## What the API allows commanding

| Group | Operations | Origin |
|---|---|---|
| Engine sessions | List, create, view, update, delete, start, stop, restart, log out. | [code] |
| Pairing | QR and code by phone number. | [code] |
| Server | Version, environment, state, stop. Health and ping. | [code] |
| WhatsApp limits | Query of the message ceiling and of the first-contact block, per session. | [code] |

Several engine sessions work in the CORE edition, one per number. **[observed]**

## Configuration of an engine session

| Field | Content |
|---|---|
| `webhooks` | Address, events, signing secret, resend policy, headers. |
| `ignore` | Ignore status, groups, channels and broadcast lists. |
| `gows.storage` | What the engine keeps: messages, groups, chats, labels, contacts. |
| `client` | The name with which the linked device appears on the phone. |
| `metadata`, `proxy`, `debug` | Free data, outbound proxy, debugging. |

## Pitfalls

| Pitfall | Consequence for the hub |
|---|---|
| Updating the session **replaces the whole configuration** and restarts the session. | The hub only applies when there is a difference, and always sends the complete configuration. |
| What is in `ignore` is not even kept by the engine. | A session created with groups ignored loses the group history of the pairing. The hub creates the session already configured. |
| The old history only arrives **at the pairing**. There is no "load older ones". | The hub's archive is the permanent place. |
| **Logging out deletes the history** the engine keeps of that session. | The hub imports before allowing a logout. |
| The history of the pairing does not become an event: it only comes out through the message query. | The hub imports by query. |
| The resending of events gives up after a while (about 2 minutes in the configuration used). | On start, the hub fetches from the engine what arrived while it was away. |
| The count of group participants always comes as zero. | The list of participants is counted. |
| The origin field of the event is always the chat, even in a message sent by the number itself. | The hub identifies the author by other fields. |
| The engine refuses an event address with an underscore in the machine name. | The hub is reached by a name without an underscore. |
| The API has no counter of unread messages. | "What is new" is calculated by the hub. |
| Editing without text and deleting without the complete identifier have dangerous behavior by default. | The hub validates before calling. |
| The interactive documentation asks for a credential of its own. | It does not serve as a catalog at run time. |

All the pitfalls above were **[observed]** or **[code]** in the checked version.

## What the engine does not do

Calls, archiving, pinning, blocking a contact, deleting a chat and buttons.

## Rule for new versions

Before changing the version of the engine, run by hand the hub's contract test against a test number:
read, list groups, send, and check the format of the message event. The hub checks the version of the engine on
start and reports when it is not a tested version.

How to run the contract test: section "Contract test", at the end of this file.

## The formats the hub uses

**Read on 11/11/2026 in the author's public documentation** (`waha.devlike.pro`, pages `docs/how-to/`
on sessions, events, sending messages, chats, contacts, groups, security, received messages
and observability). Reading only: nothing was run against an instance. The pages do not say the
version of the documentation; the versions that appear in them are only from examples (`2023.10.12`,
`2024.2.3`, `2024.3.3`, `2026.8.2`). Everything in this section is **[documentation]**, except what
says "to check": it is what the documentation does not make clear, and the hub was written on the
safest reading. The fake engine of the test kit (`arauto-testkit`, module `engine`) speaks this
format; the first round of the contract test against a real engine is what turns this section into
an **[observed]** fact.

`{s}` is the name of the engine session. `@` inside an identifier goes in the path as `%40`
("Remember to escape `@` in `chatId` and `messageId` with `%40`").

### API key and server

| Format | Origin and remark |
|---|---|
| Header `X-Api-Key` on every call | [documentation] Without a key or with the wrong one: 401, `{"message":"Unauthorized","statusCode":401}`. No address stays outside the key by default. The hub treats 401 and 403 as "key refused" and does not read the body. |
| `GET /health` → `{"status", "info", "error", "details"}`, `status` ∈ `ok`, `error`, `shutting_down` | [documentation] The hub only accepts `ok`. |
| `GET /api/server/version` → `{"version", "engine", "tier", "browser"}` | [documentation] The hub reads `version`, `engine` and `tier` and compares (`version`, `engine`) with the list of tested versions, today only (`2026.10.1`, `GOWS`). |

### Engine sessions and pairing

| Format | Origin and remark |
|---|---|
| `GET /api/sessions/{s}` → `{"name", "status", "config", "me", "engine"}`, with `me` = `{"id", "pushName"}` or null | [documentation] The response for a session that does not exist: to check. The hub reads 404 as "does not exist". |
| `POST /api/sessions` with `{"name", "start": false, "config"}` | [documentation] `start: false` postpones the start: the hub creates the session already configured and only starts it on pairing. |
| `PUT /api/sessions/{s}` with the whole configuration | [documentation] The body is a whole new configuration; a session that is not stopped is stopped and started again. It confirms the first pitfall. |
| `POST /api/sessions/{s}/start`, `/stop`, `/restart`, `/logout` | [documentation] `start` and `stop` are idempotent; `stop` does not log out of WhatsApp; `logout` logs out and keeps the configuration. |
| `DELETE /api/sessions/{s}` | [documentation] Logs out of WhatsApp and deletes configuration and data. The hub calls `logout` and then `DELETE`, and only with the explicit word of the admin. |
| States: `STOPPED`, `STARTING`, `SCAN_QR_CODE`, `WORKING`, `FAILED`, `PASSKEY_REQUIRED`, `PASSKEY_CONFIRMATION_REQUIRED` | [documentation] An unknown state is not guessed: the response counts as malformed. |
| `GET /api/{s}/auth/qr?format=raw` → `{"value"}` | [documentation] The value becomes a QR in the terminal of the hub's machine and is not kept. |
| `POST /api/{s}/auth/request-code` with `{"phoneNumber"}` → `{"code"}` | [documentation] |
| Restriction on the session name | To check: the documentation only says the name works as an identifier. The hub draws from the narrowest alphabet, `^[a-z][a-z0-9]{15,62}$`. |

### The configuration of each engine session

| Field | Origin and what the hub puts |
|---|---|
| `webhooks[]`: `url`, `events`, `hmac.key`, `retries` (`policy`, `delaySeconds`, `attempts`), `customHeaders` (`[{name, value}]`) | [documentation] A single destination: the hub's inbox. Events `session.status`, `message.any`, `message.reaction`, `message.edited` and `message.revoked`. Resend `exponential`, 2 s, 7 attempts. No extra header. The page does not give the default values of the resend. |
| `ignore`: `status`, `groups`, `channels`, `broadcast` | [documentation] `status`, `channels` and `broadcast` on; `groups` off, because the group history only comes at the pairing. |
| `client`: `deviceName`, `browserName` | [documentation] `deviceName` = `Arauto`. |
| `gows.storage`: `messages`, `groups`, `chats`, `labels`, `contacts`, `messageSecrets` | [documentation] The first five on. `messageSecrets` is not sent: the default is to check. |
| `metadata`, `proxy` (`server`, `username`, `password`), `debug` | [documentation] `{}`, no proxy, `false`. A proxy put from outside is fixed. |
| Does the signing key appear back in the reading of the session? | To check. When it appears, the hub compares it in constant time; when it does not appear, it trusts the digest of what it itself applied last. |

### Events (the hub's inbox)

| Format | Origin and remark |
|---|---|
| `X-Webhook-Hmac`: HMAC of the raw body, with the configured key; `X-Webhook-Hmac-Algorithm`: `sha512` | [documentation] The encoding of the signature is not written; the example looks hexadecimal. The hub accepts only hexadecimal of 128 characters and refuses another named algorithm. The configured key is the hexadecimal text of 32 randomly drawn bytes; to check whether the engine uses it as text, as the hub supposes. |
| `X-Webhook-Request-Id`, `X-Webhook-Timestamp` (milliseconds) | [documentation] The hub does not depend on them: the time comes from the envelope, which is what the signature covers. |
| Envelope: `id` (`evt_` and a ULID in lowercase), `timestamp` (milliseconds), `event`, `session`, `metadata`, `me` (`id`, `pushName`), `payload`, `engine`, `environment` (`version`, `engine`, `tier`) | [documentation] The hub requires `id`, `timestamp`, `event`, `session` and `payload`; the rest is optional. |
| `session.status`: `payload.status` and `payload.statuses` (`[{status, timestamp}]`) | [documentation] The connected phone comes from the `me.id` of the envelope. |
| `message` and `message.any`: `id`, `timestamp` (seconds), `from`, `fromMe`, `source`, `to`, `body`, `hasMedia`, `media` (`url`, `mimetype`, `filename`, `error`), `ack`, `vCards`, `_data` | [documentation] `participant` (the author in a group) and `replyTo` are not in the example of the event: to check. `replyTo` appears in the example of the reading of messages. |
| `message.reaction`: `id`, `from`, `fromMe`, `participant`, `to`, `timestamp`, `reaction` (`text`, `messageId`); empty `text` removes the reaction | [documentation] |
| `message.edited`: `id`, `editedMessageId` (without the chat), `body`, `_data` | [documentation] |
| `message.revoked`: `after` (`id`, `_data`), `revokedMessageId` (without the chat), `before` (almost always null) | [documentation] |
| Resend: `constant`, `linear`, `exponential` (with 20% of variation) | [documentation] The page does not say when it gives up; the "about 2 minutes" of the pitfalls is [observed] in the checked version. |

### Reading

| Format | Origin and remark |
|---|---|
| `GET /api/{s}/chats/overview?limit&offset` → `[{"id", "name", "picture", "lastMessage", "_chat"}]` | [documentation] The default order is not written. |
| `GET /api/{s}/chats/{chat}/messages?limit&offset&downloadMedia=false&filter.timestamp.gte&filter.timestamp.lte` | [documentation] The time filters are in seconds (the example is `1727745026`). The default order is not written: the hub sorts from the newest to the oldest. |
| A message read: `id`, `timestamp`, `from`, `fromMe`, `body`, `hasMedia`, `ack`, `ackName`, `replyTo`, `_data` | [documentation] |
| Message identifier: `false_{chat}_{id}`, and in a group `false_{group}_{id}_{author}` | [documentation] The `true_` (sent by the number) does not appear in the message examples, only in the name of a file: to check. The hub requires the complete form to edit and to delete. |
| `GET /api/{s}/chats/{chat}/messages/{id}?downloadMedia=true` → `media.url` = `.../api/files/{file}`, downloaded with `X-Api-Key` | [documentation] The example is `/api/files/true_..._....jpg`, with `@` in the name. The hub never follows the address: it takes only the path, under `/api/files/`, in a narrow alphabet, and asks for it from the engine it knows, with the key. |
| `GET /api/contacts?contactId&session` → `id`, `name`, `pushname`, `shortName`, `isMyContact` | [documentation] It does not bring "business account": the hub returns that data as unknown. |
| `GET /api/contacts/about?contactId&session` → `{"about"}`; `GET /api/contacts/profile-picture?contactId&session` → `{"profilePictureURL"}` | [documentation] |
| `GET /api/{s}/groups?limit&offset` | [documentation] "It depends on the internal mechanism": the hub accepts a list or an object by identifier. The count of participants is that of the list (pitfall). |
| `GET /api/{s}/groups/{group}`; `GET /api/{s}/groups/{group}/participants/v2` → `[{"id", "role"}]`, roles `left`, `participant`, `admin`, `superadmin` | [documentation] The format of the group's response (subject, description) is not on the page: to check. A participant may come as `@lid`. |

### Sending and changing

| Format | Origin and remark |
|---|---|
| `GET /api/contacts/check-exists?phone&session` → `{"numberExists", "chatId", "pn"}` | [documentation] The page says that Brazilian numbers from before 2012 do not have the ninth digit and tells to check before writing: the hub tries both forms. |
| `POST /api/sendText`: `session`, `chatId`, `text`, `reply_to`, `mentions`, `linkPreview` | [documentation] The mention also needs to be in the text (`@number`): that stays with whoever writes. The response (the identifier of the sent message) is not on the page: to check; without an identifier, the hub returns "sent" without it. |
| `POST /api/sendImage`, `/api/sendFile`, `/api/sendVoice`: `file` (`mimetype`, `filename`, `data` in base64), `caption`; voice with `convert` | [documentation] Only by content in the MVP; the hub never sends `file.url`. |
| `POST /api/sendSeen`: `session`, `chatId`; without `messageIds`, it marks all the unread ones | [documentation] |
| `POST /api/startTyping`, `POST /api/stopTyping` | [documentation] The body is not on the page: to check. The hub sends `session` and `chatId`. |
| `PUT /api/reaction`: `session`, `messageId`, `reaction` | [documentation] "Reaction API uses PUT, not POST"; the table on the same page says POST. The hub uses PUT. |
| `POST /api/forwardMessage`: `session`, `chatId`, `messageId` | [documentation] It may fail for a chat that never received a message. |
| `PUT /api/{s}/chats/{chat}/messages/{id}`: `text`, `linkPreview` | [documentation] The danger of editing without text is in the pitfalls ([code]); the page does not speak of it. |
| `DELETE /api/{s}/chats/{chat}/messages/{id}` | [documentation] |
| WhatsApp limits on the session (`me.messageCapping`, `me.reachoutTimelock`) | To check: the names come from the [code] of the checked version, not from the documentation. The hub only says whether they exist. A 429 from the engine is read as a WhatsApp limit. |

### Contract test

It is in `crates/arauto-hub/tests/whatsapp_contract.rs`, marked `#[ignore]`: it never runs in the
normal suite. It only talks to a test engine and a test number, and only reads, unless the sending
is requested.

```
cargo test -p arauto-hub --test whatsapp_contract -- --ignored --nocapture
```

| Variable | What for |
|---|---|
| `ARAUTO_CONTRACT_ENGINE_URL` | The address of the test engine, `http://host:port`. |
| `ARAUTO_CONTRACT_ENGINE_KEY` | Its API key. |
| `ARAUTO_CONTRACT_SESSION` | An engine session already paired with the test number. |
| `ARAUTO_CONTRACT_PHONE` | The test phone of that session, for the check of the two forms. |
| `ARAUTO_CONTRACT_SEND_TO` | Optional: the test phone that receives a text and a reaction. Without it, nothing is sent. |

It checks the health and the version, the session, the chats, the messages of a chat, the groups
with the participants, a contact and the check of the phone, and, with the sending requested, the
text and the reaction. The format of the message event is still checked by hand, looking at what
arrives at the hub's inbox.

## Hidden identifiers in the list of chats (17/11/2026)

[observed] WAHA 2026.10.1, engine GOWS, six paired sessions of one installation, read-only, counts only.
`GET /api/{s}/chats/overview?limit=300` listed every chat with a person under an identifier ending in
`@c.us` (662 chats in all) and none ending in `@lid`; groups end in `@g.us`. `GET /api/{s}/lids/count`
answered a count above zero in every session (from 4 to 2,429), so the engine keeps a table from hidden
identifiers to telephones. Not tried: `GET /api/{s}/lids/{lid}` itself, the identifier that arrives in the
event of a message, another engine or version.

[observed] same day, same installation, five sessions, shapes only. In `chats/overview` a chat with a
person is listed under `@c.us`, but the field `from` of its last message ended in `@lid` in most chats
(127 of 143 looked at) and in `@c.us` in the rest: the same engine names one person both ways.
`GET /api/{s}/lids?limit=1` answers rows `{"lid", "pn"}` and `GET /api/{s}/lids/{lid}` answers
`{"lid", "pn"}` with `pn` ending in `@c.us`: the engine translates a hidden identifier into a
telephone. One session listed the chat of the account with itself, under its own `@c.us` identifier.
Not tried: a hidden identifier the engine does not know; the identifier in the event of a webhook.

## The engine as a service of the installation (18/11/2026)

**[observed]** on 18/11/2026, on the image
`devlikeapro/waha:gows-2026.10.1@sha256:1f7faeb0227ad5edb5711f7058bfce807ea65e5f92234af0778e4db140f6eb46`
(amd64), first alone in a throwaway container and then as the `engine` service of
`deploy/easypanel/compose.engine.yaml`, started with plain Docker Compose on a test server, with the
hub built from the code of [decision 0052](../docs/decisoes/0052-a-instalacao-traz-o-motor.md).
**Not tried on Easypanel.** The installation was removed after the run.

The image:

- Its entry point is `/usr/bin/tini --` and its command is `/entrypoint.sh`. It runs as user 0.
  `/bin/sh` is `dash`. Node is 24.
- `/entrypoint.sh` takes `WHATSAPP_API_KEY` before `WAHA_API_KEY`, unsets both, passes a value that
  starts with `sha512:` on unchanged and hashes any other.
- About 3.2 GB on disk. Idle with no number, about 410 MiB of memory; with one number paired, about
  315 MiB a few minutes later. It answered about 12 seconds after its start.

The key:

- With `WAHA_API_KEY=sha512:` followed by the SHA-512, in lowercase hexadecimal, of the text of the
  key, a call with the key in `x-api-key` answers 200; with no key, with another key and with the
  digest itself in place of the key it answers 401. This holds for `GET /api/server/version`,
  `GET /api/sessions?all=true` and `GET /api/keys`.
- `GET /ping` answers 200 with no key.

With the thirteen variables the recipe pins (the two screens off and their two "no password"
switches on, the apps, the metrics and the debug mode off, QR printing off, the general log level
`info` and the level of the access lines `debug`):

- `/dashboard`, `/api-json` and `/metrics` answer 404; `/api` and `/health` answer 401 with no key.
- The log held no line with a password, a generated value, the key or its digest, no QR and no
  access line for the calls made.
- After a pairing the log held the telephone of the account once, in the line that says the
  pairing worked, and no text of a message.

As a service beside the hub:

- A new named volume mounted on a folder the image of the hub owns took that owner and mode
  (10001, 755); the hub wrote the 135 bytes of the digest with mode 644; the engine, with the volume
  read-only, read it and could not write there.
- The hub recorded the engine by itself, the engine came up after the hub was healthy, the hub read
  the version as tested, and a number was paired in the panel with nothing typed about the engine.
- Restarting only the hub left the container of the engine as it was, not restarted, and the number
  connected.
- With the digest file removed and the engine restarted, the engine waited: one line in its log, no
  answer on its port. When the hub started again it wrote the file, the engine came up and the
  number connected again without a new pairing.
- With the digest of another key in the file and the engine restarted, the hub was refused and said
  the engine needs a restart. The alert of the refused key did not appear in the 95 seconds watched;
  it was there after the hub started again. How long the hub takes to raise it while running was
  not measured.
- Right after the pairing the engine logged once that it dropped an event for a slow listener.
  Whether the hub recovers what that event carried was not checked.

Not tried: Easypanel (one volume shared by two services, a replaced entry point, Restart and Stop,
a Deploy that needs a migration); the arm64 image; sending and receiving a message; raising the
pinned version; a stop of the engine while it waits.
