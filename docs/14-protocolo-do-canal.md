# 14 — Channel protocol

**Status: proposal**, to be validated in the first MVP ([15-mvp.md](15-mvp.md)). Revised on 10/11/2026,
after the design review. It is the format of what passes between the connector and the hub: the part
that [04-seguranca-conexao.md](04-seguranca-conexao.md), section 7, left for the prototype.

This document changes no rule of [04](04-seguranca-conexao.md). Where the two diverge, 04 holds,
and the error is here. What the review found that asks for a change in 04 is in section 12.

**The hub side already exists in the code** (stages 2 and 2b of the MVP, 11/11/2026). Where this text
was silent, the code chose a reading, and it is written here with the mark "in the hub". It is fact
only in the sense that the hub's suite checks it, on Windows, against a test PostgreSQL and a test
connector: the real connector does not exist yet, and nothing was run on Linux. The points where the
code and the text have not agreed yet are in section 13.

## Summary

```
 websocket (binary)
   └─ Noise XK            handshake in 3 messages; after that, encrypted frames
        └─ frame          1 byte of flags + 4 bytes of the stream + chunk
             └─ message   JSON with the field "t" (the type)
```

| Layer | Choice | Why |
|---|---|---|
| Transport | Websocket, binary messages only, no compression | It passes through an ordinary proxy. The content already goes encrypted: compressing gains nothing and opens a side channel. |
| Cipher | Noise `XK_25519_ChaChaPoly_BLAKE2s`, prologue `arauto/1` | Decision 0012. |
| Message | JSON in UTF-8 | The tool calls and responses are already born and end in JSON. |

Three rules run through the document:

1. **Before the session, almost nothing is accepted.** Whoever has not yet proved to be an active
   device only sends small messages, one at a time, with a deadline.
2. **Only the newest connection of a device is judged.** An abandoned attempt that arrives late
   through the network never decides anything.
3. **Size never brings down the receiver.** The sender checks first; what does not fit becomes an
   error of the call, not a closing of the connection.

## 1. Hub addresses

All on the port of the connectors.

| Address | What for |
|---|---|
| `GET /v1/channel` | The websocket. It serves the enrollment, the channel and the closing request. |
| `GET /v1/download/{file}` | The connector, in an exact version. Public and with no secret. It only serves names from a fixed list, built at startup. |
| `GET /v1/join/{handle}` | **Proposal, built, not tried on a real Hermes.** The instruction page of one invite: English only, Markdown text. See "The instruction page" below. |
| `GET /llms.txt` | **Proposal, built, not tried on a real Hermes.** A short fixed text: what this address is (not the panel), that connection pages are made one per invite in the panel, and, when it is known, where the source is. |

- A read request does not change state: opening any of them does not spend an invite.
- **There is no address that hands over the hub key.** The connector only accepts the key that came
  in the sentence or the block, and only discovers the versions through the `hello`. Whoever installs reads the
  public key in the admin command. The instruction page does not hold it either.
- The connector validates the TLS certificate of the address against the public authorities. A
  `ws://` address, without TLS, is only accepted for `127.0.0.1` and `localhost`, in development. A
  private authority is left for after the MVP.
- Health and metrics stay on another port, which is not published ([15](15-mvp.md), section 6). In
  the hub, the internal port answers only `GET /health`, with `ok`; the connectors' port does not
  answer it.
- **One exception, in the code and tested on 13/11/2026** (`channel::ports`, with the test hub):
  `GET /` on the port of the connectors answers 200, `text/plain`, with a fixed text of three
  lines that says the address is for the connectors, that the panel has its own address (the one
  set in `ARAUTO_PANEL_URL`) and a pointer for the person the Open button of Easypanel brought
  there (the wording is being corrected: the right advice is to make the panel's domain the
  primary one, or to open the panel's address directly; see
  [0040](decisoes/0040-painel-dentro-do-hub.md), pending decision 5). It names no host, no port
  and no version;
  the text carries no state of the hub. It exists because a person who opens the connectors'
  domain by mistake (for example from the Open button of Easypanel) finds out where to go.
- **The instruction page, in the hub** (proposal; the hub's suite checks it, nothing was tried on a
  real Hermes). Decision [0042](decisoes/0042-frase-de-conexao-e-pagina-de-instrucoes.md).
  - **The handle** is not the invite: 16 bytes from the system generator, written as 26 lowercase
    base32 characters, born with the invite. The hub keeps only its digest, SHA-256 of
    `"arauto join page\0"` followed by the 16 bytes, and erases it with the invite. It is never
    logged: the router logs no path.
  - **The page is a fixed text compiled into the hub.** Its only fields have a strict format: the
    host of the hub (from `ARAUTO_PUBLIC_URL`, never from the request), the name of the Hermes, the
    UTC deadline, the version and file name of the connector, and, when it is known at build time (not today: the
    `repository` metadata is pending, decision 0042), the public address of the project's source. It never holds the invite, the hub key or the connector digest: those travel
    only in the person's sentence, and the page tells the agent to recognize each by its form.
  - **When it serves.** While the invite is `issued` and not expired, or `used` with its device
    `pending` and the verification deadline not passed. The hub erases the digest when the invite
    is cancelled, expires or is superseded, and when the request is approved, denied, expires or is
    closed. A new invite for the same Hermes kills the old page.
  - **What a GET does.** It only reads: one query, no lock; it never spends or changes the invite.
    Unknown, malformed, expired, cancelled or decided handles get one 404 with a fixed text,
    byte-identical in every case. Other methods on these addresses get today's empty 404.
  - **Headers** on the page, on the 404 and on `/llms.txt`: `Cache-Control: no-store`,
    `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `X-Robots-Tag: noindex`.
    Page and `/llms.txt` are `text/markdown; charset=utf-8`; the 404 is `text/plain`.
  - **Ceiling.** At most 4 lookups at once; beyond that, 503 with a fixed text. A hub with no
    connector to serve answers the same 404 for every handle.
  - **What is not guaranteed.** The address can leak (preview services, access logs of a proxy). It
    reveals the name of the Hermes, the deadline and, by the switch from 200 to 404, roughly when
    the request ended. It does not reveal the invite. Whether a real Hermes accepts the page is
    pending ([05](05-modelo-de-ameacas.md)).
- In the hub, every other address, and every method other than `GET`, receives 404 (`GET /` on
  the internal port, too: tested). This holds for the port of the connectors and for the internal one. The
  panel port, when turned on, is a third one: its addresses do not exist on those two, nor theirs
  on the panel's
  ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)).
- **Download, in the hub.** The requested name is only compared with the names of the list and never
  becomes a file path: `../`, a slash, an encoded character or a letter with its case changed simply
  find nothing. A name of the list has up to 128 letters, digits, dots, hyphens and underscores, and
  does not start with a dot. In M1 the list is empty: every download is 404.

Rules of the websocket:

- Binary messages only. A text message closes the connection.
- Each message has at most 65,535 bytes. The hub sets this ceiling in the library itself: a larger
  message is not even assembled.
- The compression extension is not negotiated.
- The hub gives 5 seconds for the request headers to arrive. In the hub, the deadline also holds
  between two requests of the same connection: the HTTP connection stays open between them, because
  closing it would break the switch to websocket in the library used.
- The `ping`s of the websocket itself are answered by the library and do not count as a sign of
  life. The heartbeat that counts is the protocol's (section 4).

## 2. Handshake

The connector is the one that starts. Its static key is the device identity; the hub's came in the
block.

```
 connector -> hub   e, es            48 bytes
 hub -> connector   e, ee            48 bytes
 connector -> hub   s, se            64 bytes
```

- The three messages go without payload and have an exact size. Another size closes the connection.
- The prologue is `arauto/1` and **does not change** with the protocol version.
- A hub that does not have the key of the block does not conclude: the connector closes with AR-202.
- A key that belongs to no one (a low-order point, with which every computation gives zero) is
  refused on both sides: neither device identity, nor hub key.
- The hub gives 10 seconds, counted from the opening of the websocket, for the handshake to finish.
  Past that, it closes without answering.
- The **handshake digest** is the value `h` of Noise at the end of the third message (32 bytes).
  Both sides read it before moving on to the frames. It goes into the verification code (section 8).
- The XK pattern does not bar an unknown identity: anyone who has the hub's public key concludes the
  handshake. That is why the limits of section 4 hold for every connection.

## 3. Frames

After the handshake, each binary message of the websocket is a Noise transport message.

- Each direction has a counter, which starts at zero and goes up by one at each frame. The counter
  **does not travel**: the receiver uses its own. A frame that is repeated, missing or out of order
  does not decrypt, and the connection is closed.
- **Only the task that writes to the websocket encrypts**, at the time of writing, after choosing
  between the control queue and the data queue. Encrypting when enqueueing would break the counter on
  the first frame that jumped ahead.

The decrypted content of a frame:

```
 byte 0      flags: bit 0 = END (last chunk of the message); bit 1 = CONTROL
 bytes 1-4   stream number, unsigned, from the most significant byte to the least
 bytes 5...  chunk of the message
```

| Rule | Detail |
|---|---|
| Bits 2 to 7 | Always zero. Another value closes the connection. |
| Single-frame message | It goes in **stream 0**, with END. It does not count as an open stream. |
| Larger message | It is cut into chunks of up to 60,000 bytes, all with the same stream number; the last one carries END. |
| Empty chunk | Error: closes the connection. |
| Numbering | The connector uses the odd numbers and the hub the even ones from 2 on, always growing. A number of the wrong parity, repeated or smaller than the last one opened by that side closes the connection. |
| Interleaving | Different streams may interleave. The message is delivered when the frame with END arrives. |
| CONTROL | Heartbeat and check. Always a single frame, in stream 0, with END. It jumps ahead of the data queue. |
| The sender | Only opens a stream with the whole message ready, and waits for room if it already has 8 open. It never exceeds the limit on purpose. |
| End of the numbers | When the stream numbers of one side run out, it closes the connection with AR-205 and reconnects. |

A message is interpreted only once. Neither side passes the bytes on as it received them: the hub
builds what it sends to the engine from the already validated arguments.

## 4. Limits and deadlines

The limits exist because the connection is open to anyone until the session starts.

| Phase | What holds |
|---|---|
| Websocket | Message of up to 65,535 bytes; binary only. |
| Handshake | Exact sizes (48, 48, 64); 10 seconds. |
| Before `session` | Only stream 0. Message of up to 4,096 bytes. The `hello` in 5 seconds. Each response of the connector in 10 seconds; the `saved`, in 60. Apart from the waiting of the request, the whole connection has 90 seconds to become a session. |
| Waiting | The pending device only sends `check_ok`, `ping`, `pong` and `leave`. |
| In session | From the connector to the hub, message of up to 6 MiB. From the hub to the connector, up to 16 MiB. Eight open streams **per direction**, counted by the receiver. An open stream that stays 60 seconds without a new chunk closes the connection. |
| Calls | Up to 32 in progress per connection. The next one is refused with AR-710, without closing. |
| Content of a message | At most 100,000 JSON values and 64 levels of nesting. Without this, a message of a few MiB made only of zeros would cost dozens of times its size in memory. The sender checks first: it becomes AR-709 of the call. |
| The whole hub | A budget of bytes for messages in reassembly (256 MiB to start with). With no room, the connection that asks for more is closed with AR-205. |
| Connections without a session | At most 1,024 in the hub and 16 per origin, apart from those that are waiting. Above that the hub answers 503 or 429 before opening the websocket. |

How the hub applies the table:

- **The 90 seconds** count from the opening of the websocket, with the handshake inside, and stop
  while the connection is waiting.
- **The origin** is the IPv4 address, or the first 64 bits of the IPv6 address (the network of a
  home or of a carrier). An IPv4 written as IPv6 counts as the IPv4. Where the address comes from is
  in [15](15-mvp.md), section 4.
- **Outside the ceiling**: the waiting connection and the one that is in session. A TCP connection
  that has not sent the headers yet does not count either: only the 5-second deadline limits it.
- **A message that only the hub sends**, coming from the connector, closes the connection, before or
  after the session.
- **The budget of the whole hub**: the room a connection took from it comes back when it ends.
- **Large messages**: above 256 KiB, the hub interprets the JSON outside the network tasks.
- **Writing.** Each connection has two output queues: control, with 16 frames, and data, with 64.
  Whoever produces data waits for room. A full control queue closes the connection without
  `closing`. A frame that does not go out in 30 seconds also closes it: the deadline is per frame,
  and not "no byte moved in 30 seconds".
- **The code.** Silence of 90 seconds, `hello` past the deadline, response past the deadline and
  `ping` or `pong` before the `hello_ok` close with AR-204.

What does not fit:

- Whoever builds a message checks the size before sending.
- A response that does not fit becomes `result` with `ok: false` and AR-709. The reads have a budget
  in bytes and cut the list at whole items ([09](09-contrato-mcp.md), [13](13-conversa-entre-hermes.md)),
  so AR-709 only appears for a single item that is too large by itself.
- The connector refuses locally, with AR-709, the call whose arguments do not fit.

Deadlines on the connector's side, always larger than the sum of the hub's:

| Wait | Deadline |
|---|---|
| `hello_ok` and `state` | 10 seconds each |
| `check`, after the `present` | 15 seconds |
| `credential`, after the `check_ok` | 20 seconds |
| `session`, after the `saved` | 20 seconds |
| `enrolled` | 15 seconds |
| `left` | 10 seconds |

**The connector ends one attempt before opening another** and, after closing an attempt in which it
already sent `present` or `enroll`, waits 6 seconds to open the next one with the same identity. The
`join` holds a lock on the folder while it runs: two at the same time do not happen.

Heartbeat: each side sends `ping` every 30 seconds, from the `hello_ok` on, including while waiting.
Any valid frame received counts as a sign of life. Ninety seconds without receiving anything end
the connection, with AR-204.

Reconnection: after a drop, the first attempt is immediate (C9). The following ones wait a time
drawn at random between zero and the ceiling, which doubles at each failure, from 1 to 60 seconds.
Without the random draw, the connectors refused in a mass return would all try again together.

Every deadline of this section comes from a configuration and from a clock that the tests control.

## 5. Messages

Every message is a JSON object with `"t"`. An unknown field is ignored. A repeated key is an error.
Binary data goes in standard base64, with padding.

A message of unknown type: before the session, it closes the connection; after, it is ignored, and
the hundredth closes with AR-204.

### 5.1 The first message, and what never changes

The first message is the connector's:

```json
{"t":"hello","proto":[1,1],"connector":"0.1.0","purpose":"channel"}
```

| Field | Content |
|---|---|
| `proto` | The lowest and the highest protocol version the connector speaks. |
| `connector` | The version of the program. It is declared text: it appears on the screen and decides nothing. |
| `purpose` | `enroll` (enrollment), `channel` (channel) or `leave` (closing request). Another value: `closing` with AR-204. |

The hub answers `hello_ok`:

| Case | Response |
|---|---|
| There is a version in common | `{"t":"hello_ok","proto":N}`, with the **highest** common version. |
| The connector is too old | In `channel`: `{"t":"hello_ok","proto":0,"outdated":true}` and the presentation without swap follows (5.3). In `enroll`: `closing` with AR-401. In `leave`: it serves it. |
| The connector is too new | `closing` with AR-402, in any `purpose`, including `leave`. |

In the hub, the `leave` of a connector that is too new also receives AR-402: there is no valid
`hello_ok` for a range of versions entirely above the hub's. C8 says that the `leave` is accepted
"in any version"; the hub's reading is that the exception holds for the connector that is too old
(section 12).

What decides AR-401 is the version of the **protocol**. The list of versions of the program, with
revocation, is left for after the MVP.

**Frozen forever**, so that a connector of any age still receives the right error: the handshake
and the prologue, the frame header, and the messages `hello`, `hello_ok`, `state`, `present`,
`check`, `check_ok`, `closing`, `leave` and `left`.

### 5.2 Enrollment (`purpose: enroll`)

| From | Message | Fields |
|---|---|---|
| connector | `enroll` | `invite`; `declared`, with `hostname`, `os` and `hermes_version` (untrusted text, of one line, up to 128 characters each, with no control or direction character) |
| hub | `enrolled` | `nonce` (32 bytes), `confirm_within_s`, `enrollment` (from 1 to 5) |
| hub | `closing` | `code`: AR-101, AR-102, AR-104, AR-301, AR-302 or AR-303 |

- The `enroll` is only sent after the `hello_ok`. Before that, or repeated, it closes the connection.
- After `enrolled`, the connector computes the code, shows it and closes. It is a short connection.
- The enrollment refusals are recorded **aggregated**: one row per origin, organization, code and
  minute, with the count. One row per attempt would let anyone fill a table that is not erased. In
  the hub this holds for every refusal, and not only for the AR-101 of whoever has no device: the
  repeated AR-102 is also counted this way. The count stays in memory until the minute closes, and
  the sweep writes it. **Limit:** what was counted in the minute in which the hub stops is lost.
- The invite with no organization (a digest that no one issued) receives AR-101 and enters the count
  with no organization.
- In the hub, an expired invite that the enrollment finds is closed right there, with the event
  `invite.expired`, without waiting for the sweep.
- An invite not yet used, presented by an identity that already has a device row, of any Hermes,
  receives AR-101 and is not spent: an old identity does not open a new request.
- A request already resolved answers AR-104 in every state that only exists after an approval:
  active, quarantine, revoked, replaced and expired by inactivity.

### 5.3 Channel (`purpose: channel`)

After the `hello_ok` **the hub always speaks first**, with `state`.

| `state` | When | What follows |
|---|---|---|
| `waiting` | Pending device. | The hub sends `check`. Once answered, the connection is **waiting**. When the request is resolved, the hub sends another `state` or a `closing`. |
| `approved` | Approved device, at generation zero. | The swap (section 6). |
| `active` | Active device. | The swap (section 6). |
| `ended`, with `code` | Any other state, unknown identity, or connector that is too old. | The **presentation without swap**: the connector presents what it has, answers the check and receives `closing` with the code. The hub never issues a credential here. |

The presentation without swap exists because 04 requires looking at what a closed device presents
(C7: "reuse after closing"; C11: a seal the database does not explain) and because decision 0030
requires checking the credential of the old connector before refusing the version. A connector that
is too old presents itself this way at the leader's startup and, after that, once every 24 hours.

| From | Message | Fields | When |
|---|---|---|---|
| hub | `state` | `state`, and `code` in `ended` | Always the first after the `hello_ok`; again when the waiting ends. |
| connector | `present` | `credential` (up to 1,024 bytes) or `null` | In `approved`, `active` and `ended`. `null` when it has no credential. Out of this order, it closes. |
| hub | `check` | `nonce` (16 bytes) | The check of rule 3 of C7. CONTROL flag. |
| connector | `check_ok` | `nonce` | The same one. CONTROL flag. |
| hub | `credential` | `generation`, `sealed` | The new credential, already written as pending. |
| connector | `saved` | `generation`, `digest` (SHA-256 of the block received) | The "saved", after the write is guaranteed on disk. |
| hub | `session` | `hermes` (name), `tools` (names released), `wake` (may be woken) | The promotion was written. The channel session starts here. |
| hub | `tools` | `tools` | The list of released tools changed. |
| connector | `call` | `id`, `tool`, `args` | Tool call. |
| hub | `result` | `id`, `ok`, and `content` or `code`; optional `retry_after_s` | Response. A refusal carries only the code; the sentence stays in the connector. |
| hub | `notice` | `id`, `kind`, and the fields of the kind | Notice: a pointer, with no third-party text. |
| connector | `notice_ack` | `id`, `outcome` | After Hermes's local API accepts. |
| hub | `rotate` | | The 24-hour swap starts. |
| hub | `rotated` | `generation` | The 24-hour swap was written. |
| both | `ping`, `pong` | `n` | Heartbeat. The `pong` returns the `n`. CONTROL flag. |
| connector | `leave` | | Closing request through the open channel, waiting or in session. |
| hub | `left` | `status` | Response to the `leave`. |
| hub | `closing` | `code` | The reason for the closing (section 7). |

**Calls.**

- `call` before `session` is a protocol error: the connection is closed. It is rule 4 of C7.
- The `id` is a positive integer, from the connector, unique among the calls in progress of that
  connection. Repeated, it closes the connection.
- The hub validates again the name of the tool and all the arguments: the connector may be hostile.
  A tool that does not exist or a malformed argument: AR-711. A tool not released: AR-701.
- `retry_after_s` is an integer from 1 to 86,400. The connector validates it and fits it into the
  fixed sentence of the error.
- A call that did not get to go out when the channel drops goes back to the LLM with AR-203: it may
  be repeated. A call that went out and was left without a response goes back with AR-207: the
  result is unknown, and the sentence says to check before repeating. The hub does not recognize a
  repeated call.

**How the hub serves the session:**

- **A message sent before reading the `session`.** Every message the connector sent before the
  `session` left the hub (a `call`, a `leave`, any other) closes the connection with AR-204, after
  the `session`. The hub knows the exact order: the connection moves to the session at the instant
  the `session` leaves the send queue. The promotion is already written, and the connector comes
  back with the new credential.
- **The order of each call:** first the hub rereads the state of the device (C8). If it no longer
  serves, the response is `result` with the code of the state (for example, AR-502), followed by
  `closing` with the same code. Only then come the name and the arguments (AR-711), the release
  (AR-701) and the size of the response (AR-709).
- **The repeated identifier** closes with AR-204. An identifier leaves the calls in progress before
  its `result` is sent.
- **The 33rd call in progress** receives AR-710, sent by the session's own loop: a connector that
  insists waits for the refusals to go out, and the wait reaches its connection. This refusal
  **does not reread the state of the device** (section 12).
- **In M1** only `arauto_status` is released, and the hub answers it without the engine. The fields
  are in [09](09-contrato-mcp.md), section 3. The `hermes_*` ones exist in the catalog and receive
  AR-701; the `whatsapp_*` ones are not in the catalog yet and receive AR-711. The `tools` message
  is never sent, because the list does not change, and no `notice` goes out; a `notice_ack` is
  ignored.
- **`saved` outside the 24-hour swap** closes the session with AR-204 (C7, rule 7).
- **The `leave` through the open channel** receives `left` and the connection ends.

**Notices.**

| `kind` | Fields | Validation in the connector, before building the fixed sentence |
|---|---|---|
| `whatsapp` | `number` (alias), `chat` (opaque identifier), `count`; the `id` of every notice is a UUID | `number` in the name format; `chat` with up to 64 characters of `A-Z a-z 0-9 _ -`; `count` from 1 to 9,999 |
| `peer` | `from` (name of the Hermes), `count` | `from` in the name format; `count` from 1 to 9,999 |

The name format is `^[a-z0-9][a-z0-9_-]{0,31}$`, and it holds for the name of the Hermes and for the
alias of the number. The validation is **compiled into the connector**: even an invaded hub only
manages to put into the sentence a name in this format and two numbers.

| `outcome` | When | What the hub does |
|---|---|---|
| `accepted` | The local API accepted. | Takes the notice as confirmed. |
| `duplicate` | The connector had already handled that `id`. | Takes it as confirmed. |
| `failed` | The local API did not answer or refused. | Tries again, with a growing wait, until the notice expires. |
| `throttled` | The connector reached its own ceiling of wakes. | Tries again in 5 minutes. |
| `rejected` | Unknown kind or field out of format. | Discards the notice. Never tries again. |

### 5.4 Closing request

| From | Message | Fields |
|---|---|---|
| connector | `leave` | |
| hub | `left` | `status`: `closed`, `already_closed`, `unknown` |

- With the channel open, the request goes through it (C10). With no channel, it goes through a short
  connection with `purpose: leave`, one per identity. A short connection does not present a
  credential and does not count as a double connection.
- The three values count as confirmation for the connector to erase the folder of that identity.
- A pending request that asks for the closing becomes denied.
- In the hub, an active device that leaves becomes revoked, with the alert of closing requested by
  the device and the address it came from (C10).
- **If the database came back from a backup** and the hub discovers this in the middle of the
  `leave`, nothing is written: the hub turns on the restored mode (section 6, "Restored mode with
  the hub running"), and the short connection closes with AR-205, without `left`.
- **If the audit mark did not advance** after the closing is written, the hub writes the alert
  `mark_not_advanced` for the admin and only then answers `left` with `closed`, because the closing
  is in the database. The alert comes first so that it is not lost if the hub goes down between the
  two: after the `left`, the connector erases the folder.

## 6. The swap, step by step

The hub keeps, in memory, the registry of the channel connections of each device. Each connection
receives an **order number** at the end of the handshake. A connection is **established** when it is
waiting or in session.

```
 1 hub        state
 2 connector  present                       (while waiting there is no present)
 3 hub        check        deadline: 5 s    no response: closes, with no consequence
 4 connector  check_ok
 5 hub        is this the newest connection of the device?   no -> closes with AR-206, with no consequence
 6 hub        closes with AR-206 the older ones that are not established yet
 7 hub        pre-judgment, read-only (table below)
 8 hub        if the presentation would be accepted and there is an established connection:
              sends check to it and waits 3 s
                answered        -> double connection (C9; on the pending one, C6)
                did not answer  -> closes the old one with AR-208; the new one goes on
 9 hub        T1: locks, rereads, judges again, writes the pending one and the highest generation
10 hub        credential
11 connector  writes to disk and sends saved  deadline: 60 s
12 hub        T2: locks, rereads, checks and writes the promotion
13 hub        session
```

- **No transaction stays open during the network.** The waits of steps 3, 8 and 11 happen with no
  lock and no database connection. T1 and T2 are short transactions.
- **Every write that changes a device rereads the state first.** T1 and T2 lock first the row of
  the Hermes and then those of its devices, and only write if the device is still approved or
  active. In T2, only if the digest received in the `saved` is that of the pending one issued **on
  that connection**. The admin command locks in the same order. This way a device revoked
  between step 10 and step 12 does not go back to active.
- Steps 5 and 6 close the race of the abandoned attempt: if the response of an old attempt arrives
  after the connector has already opened another, it is ignored.
- A `saved` that does not refer to the pending one of that connection closes the connection (C7,
  rule 7).
- If step 8 closed a session that was serving, the alert "the connection of {name} was taken by
  another" goes out (C9).
- A connection that received `credential` and ended without `saved` is recorded. Three in 24 hours
  generate an alert (C7).

**How the hub does the steps:**

- **The registry** is a map partitioned by device identity, in the memory of one process. Each
  connection has its order number and its state: attempt, waiting or in session.
- **One attempt at a time.** Each identity has its turn to be judged, from step 5 to the end of T1.
  Right before T1 the hub checks again that that connection is still the newest and open; if not,
  it closes with AR-206 and receives nothing. Without this, an abandoned attempt that got past step
  5 could run T1 after a newer one had already swapped the credential, and send the legitimate
  device to quarantine for reuse. It was the randomized rounds that found this case.
- **The pre-judgment** (step 7) only reads. If the database fails in it, the connection closes with
  AR-205 and does not reach T1.
- **The test of step 8** is answered by the task that reads from the old connection: a `check_ok`
  with the nonce of the test does not wait for the session, which may be busy with the database. An
  old connection that is already ending is not tested. If the old one answers, both receive AR-501
  and the device goes to quarantine; on the pending one, the request is denied (AR-301) and the
  alert of copied identity goes out. If it does not answer, only the old one receives AR-208. The
  alert of taken connection only goes out if it was serving a session at the moment of the test and
  was still open at the end of the deadline.
- **The `state` goes out before the `check`.** The `check` is a control one and jumps ahead of the
  data queue; that is why the hub waits for the `state` to go out before sending it.
- **The proof of restoration** is looked at before any expired deadline, so that the deadline does
  not hide the proof.
- **On the first confirmation**, the replaced device (the old one of the same Hermes) receives
  AR-503, if it has an open connection.
- **Promotion by the presentation of the pending one.** An approved device that comes back
  presenting the pending one (it went down after writing, before the hub heard the "saved") becomes
  active already in T1, and that is the first confirmation. The session goes on only after the
  "saved" of the following credential (section 12).
- **Failures in the security write.** If T1 or T2 discover a restored database, the hub turns on
  the restored mode. If the audit mark did not advance after T2, the promotion stays written, but
  the hub does not open the session: it closes the connection and writes the alert
  `mark_not_advanced`; on the way back the connector presents what it has, and no one is locked
  out. If the connection has already closed when T1 or T2 run, nothing is written.
- **No network wait holds a transaction or a database connection**, including the waiting of the
  pending one.

**The waiting of the pending one.** The hub sends `state` `waiting` and `check`; once answered, the
connection is waiting and leaves the ceiling of connections without a session. The hub nudges it
when a notice from the admin command arrives or when the sweep sees that the request left
pending. Approved: a new `state` and the swap. Denied or expired: `closing` with the code. A `leave`
is accepted during the waiting.

**Restored mode with the hub running.** Wherever the proof comes from (the sweep, T1, the
presentation of an unknown one, a `leave`), the hub turns on the mode and closes the sessions and
the waiting connections with the code of the new state of the device (suspended: AR-506). The other
connections receive AR-205.

**The judgment**, for an approved or active device (it is the table of C7):

| The connector presents | The hub |
|---|---|
| The current one, or `null` at generation zero | Accepts. Issues a new one, which becomes the pending one; the previous pending one is discarded. |
| The pending one | Accepts. Promotes the pending one to current, the previous current one dies, and it issues another. |
| Another one already issued, or `null` after generation zero | Quarantine and alert: "reuse". |
| A seal that does not open, or of another device | Quarantine and alert: "unreadable credential". |
| A valid seal of a generation higher than all the known ones | Proof of restoration: turns on the restored mode (C11). |

In the presentation without swap the hub only looks, never issues:

| Situation | The hub |
|---|---|
| Connector that is too old, approved or active device | Applies the last three rows of the table above. Then, `closing` with AR-401. |
| Device revoked, replaced or expired by inactivity, with a valid seal older than the last current one and the last pending one | Alert "reuse after closing" (C7). Then, the code of the state. |
| Any state, valid seal of a generation higher than all the known ones | C11: proof of restoration, unless the device is already marked. |
| Unknown identity with a valid seal | C11: it enters the list of unknown devices and is proof of restoration, unless it is already listed. AR-504. In the hub, with the restored mode on, it is listed and is not proof (section 13). |
| Unknown identity with `null` or with a seal that does not open | AR-504. Nothing is written per attempt. |

Four more readings of the hub for this table:

- The seal of the unknown one is tried with the key of each organization.
- The proof of an unknown one stays kept in the row of the list, as **pending proof**, until the
  restored mode is turned on. A hub that stops between the listing and the mode finds the proof
  again at the next presentation, and turning on the mode handles all the pending proofs at once.
- An approved or active device, already marked by restoration, that presents a seal of a higher
  generation goes to quarantine for reuse; in the other states, this only generates a record. By
  C11 this combination should not even exist.
- The code of a device expired with no reason that explains it is AR-507: it is the only one that
  never leads the connector to discard anything.

**The 24-hour swap.** The hub sends `rotate`, does T1 as if the current one had been presented,
sends `credential`, waits for the `saved` for 60 seconds, does T2 and answers `rotated`. The calls
go on during the swap. Without the `saved` within the deadline, the hub closes the connection, and
on the way back the usual table holds. In the hub, each session checks the time of the swap at the
same interval as the sweep (2 seconds), counting from the last confirmed swap of that connection.
The `saved` that does not arrive within the deadline and the `saved` of another credential close
with AR-204.

## 7. Closings

The hub sends `closing` with the code before closing, whenever it can. In the hub, the `closing`
does not jump ahead of the data queue: it goes out after what was already in the queue, to keep the
order (for example, the `hello_ok` before the AR-205), and nothing enqueued after it goes out. The
writing of the whole queue has the same deadline of 30 seconds. Only the control frames jump ahead.
A connection still in the handshake, or with the control queue full, closes without `closing`.

| Code | When | What the connector does |
|---|---|---|
| AR-204 | Protocol error, limit exceeded or invalid message. | Reconnects with a growing wait. Does not repeat the same message. |
| AR-205 | The hub is stopping or out of resources. | Reconnects after 1 to 10 seconds, drawn at random. |
| AR-206 | This attempt was replaced by a newer one of the same device, before becoming a channel. | Waits from 5 to 15 seconds, drawn at random, and tries again. |
| AR-208 | Taken for dead: it did not answer the test, and another connection of the same device took its place. | Reconnects immediately (C9). If someone took its place, that is where it shows. |
| AR-301, AR-302, AR-303 | The request was denied or expired. | [10](10-maquinas-de-estado.md), section 6: only discards a folder with no credential. |
| AR-401, AR-402 | Protocol version with no agreement. | Stops using the channel and shows the error in every tool. |
| AR-501, AR-506 | Quarantine; suspended. | Stops using the channel. Erases nothing. |
| AR-502 | Revoked. | Stops using the channel. Erases nothing. |
| AR-503 | Replaced. | Starts using the folder of the new device, if there is one. |
| AR-504 | The hub does not recognize the device. | Stops using the channel. Erases nothing. |
| AR-507 | Expired by inactivity. | Stops using the channel. Erases nothing. |

"Stopping using the channel" does not end the connector ([10](10-maquinas-de-estado.md), section 6).

Orderly stop of the hub, in the reading that loses no result:

1. It stops accepting connections. A websocket request that still arrives receives 503, and the
   connections that are still in HTTP or in the handshake are cut.
2. It sends `closing` with AR-205, immediately, to each connection that is not in session.
3. Each session refuses a new call with AR-205, sends the `result` of the calls in progress and only
   then sends `closing` with AR-205.
4. After 8 seconds, it closes by force what is left, with AR-205, and waits 2 more seconds for the
   `closing` to go out. The stop may take, then, up to 10 seconds.

## 8. Verification code

```
 material = HKDF-SHA-256( salt = nonce of the hub (32 bytes),
                          key  = handshake digest (32 bytes),
                          info = "arauto/code/1" ),  with 64 bytes of output
 code     = six digits taken from the material, without bias
```

"Without bias": the material is read in 16 words of 4 bytes, from the most significant byte to the
least. The first word smaller than 4,000,000,000 is reduced modulo 1,000,000; the others are
discarded. If none serves, the hub draws another nonce before sending the `enrolled`. The code is
shown as `000-000`.

The nonce only leaves the hub after the third message of the handshake, through the already
encrypted channel (C4).

Test vectors, for two implementations to match without depending on the same library:

| Nonce (hex) | Digest (hex) | Code |
|---|---|---|
| `00` × 32 | `00` × 32 | `028-213` |
| `000102…1f` | `202122…3f` | `935-415` |

The suite of `crates/arauto-protocol` keeps both and checks them with a second implementation.

## 9. The seal

The sealed credential is a block of bytes that only the hub opens.

```
 seal          = version (1 byte, value 1) || nonce (24 bytes) || XChaCha20-Poly1305( content )
 key           = HKDF-SHA-256( key of the organization, info = "arauto/seal/1" ), 32 bytes
 authenticated = "arauto/seal/1" || version (1 byte) || id of the organization (16 bytes)
 data             || public identity of the device (32 bytes)
 content       = JSON: v, org, device (the public identity), generation,
                 id (16 random bytes), issued_at
```

- It is still "sealed with the key of the organization" (04): each use derives its own key from it,
  and the one of the notes is another ([13](13-conversa-entre-hermes.md), section 6).
- The authenticated data ties the seal to the version, to the organization and to the device: a
  seal taken to another device does not open.
- The hub keeps only the generation and the **SHA-256 digest of the whole block** of the current one
  and of the pending one, and the highest generation.
- `issued_at` is the time of issue. No rule of the MVP reads it: it stays written for the open point
  of C11.
- The connector keeps the block as received, in `credential.seal`, and never interprets it. It only
  computes the digest for the `saved`.

## 10. Invite and hub key

**Invite:** 32 random bytes, written as `arc1` followed by the base32 without padding, in lowercase:
56 characters. The hub keeps only the SHA-256 digest of the 32 bytes. The connector refuses locally,
with AR-101, an invite outside this format.

**Public key of the hub**, as it goes in the block: `arh1` followed by the 32 bytes in the same
writing, also 56 characters. There is only one text for each key. The connector refuses text outside
the format and a key that belongs to no one.

## 11. The local socket

Between the copies of the connector on the same machine ([11-conector.md](11-conector.md), section 3).

| System | How |
|---|---|
| Linux and macOS | Domain socket at `channel.sock`, inside the connector's folder. Only the owner reads and writes the folder and the socket, and the leader checks that the other side is the same user. |
| Windows | Named pipe, with the name derived from the path of the folder. The leader requires being the first instance and refuses a remote client. It is a development-only platform ([decision 0036](decisoes/0036-windows-so-para-desenvolvimento.md)). |

The leader only creates the socket after taking the lock, and erases what was left of a dead leader.

Each message is a JSON object on one line, with `"v": 1`. An unknown version or operation receives
`{"ok":false,"error":"unsupported"}`.

| From | Operation | Response |
|---|---|---|
| relay | `call`, with `id`, `tool`, `args` | `{"id":1,"ok":true,"content":{}}` or `{"id":1,"ok":false,"code":"AR-..."}` |
| relay | `tools` | The list of released tools. |
| relay | `watch` | The connection stays open, and the leader sends a line each time the list of tools changes. |
| `status` | `status` | The situation of the device and of the channel, with no secret. |
| `join` | `new_request` | The leader starts following the new request. |
| `leave` | `leave` | The leader asks for the closing of each identity of the folder and returns one response per identity. |
| `join` | `restart` | The leader ends, for Hermes to start it again in the new version. |

No operation of the socket returns the identity or the credential. In shielded mode, `leave` and
`restart` stay out of the LLM's reach.

## 12. Known limits and what is left to prove

Points the review raised and that ask for a change in [04](04-seguranca-conexao.md). They stay here
until the maintainer approves the wording:

| Point | Where it would go |
|---|---|
| The 3-second test of the old connection may fail during a large transfer on a slow network: the response waits behind what is already in the system's send queue. The old connection is taken for dead and the new one comes in, with the alert of taken connection. | C9, as a limit. |
| A valid seal, of unknown digest and already known generation, is treated as reuse (quarantine). It may also be a sign of restoration. Keeping the digest of every credential issued would tell the two apart. | C7 and C11. |
| The enrollment refusals are recorded aggregated, one row per origin, organization, code and minute, and not one per attempt. In the hub this holds for every refusal, including the repeated AR-102; C3 speaks of a "record in the audit" at each refusal. The count of the minute in which the hub stops is lost. | C3. |
| The rule that would use the date of issue of the seal is still open. | C11. |
| The first reconnection is "immediate". A random wait of a few seconds would ease the mass return after the hub restarts; today what eases it is the ceiling of connections without a session. | C9. |
| The text still speaks of a "continuous backup of the control database", from the design with two files. | C11. |

Points that came from the hub's code (stages 2 and 2b, 11/11/2026). Each one says what would change
in 04 and why; until the approval, 04 still holds as it is:

| Point | Where it would go |
|---|---|
| **Connector that is too old.** C8 says that it "receives AR-401 and nothing else". By decision 0030, already decided, the hub first does the presentation without swap (section 5.3) and only then sends AR-401. The hub does it this way. C8 would come to describe this order. | C8, and section 6, item 3. |
| **`leave` of a connector that is too new.** C8 accepts the `leave` "in any version". The hub answers AR-402 to the connector that is too new, also in the `leave`, because there is no valid `hello_ok` for a range entirely above the hub's. C8 would come to say "in any version below the hub's maximum". | C8. |
| **The AR-710 refusal does not reread the state of the device.** C8 requires checking the state "on every call". The hub rereads on every call that runs, before any other check; the 33rd call in progress is refused without touching the database. Rereading there would make each refusal compete for the pool of connections with the 32 calls in progress and would break the ceiling. The sweep closes within 2 seconds the device that no longer serves. Either C8 comes to say "on every call that runs", or the hub comes to reread on the refusal too: a decision of the maintainer. | C8. |
| **The digest of the invite is also erased on cancelling.** 04 requires erasing it when the invite expires. The hub also erases it when the invite is cancelled: by a new invite, by revoking or by the restored mode. A digest kept without use only serves whoever reads the database. | C1 and section 4. |
| **In which write the approved one becomes active.** 04 does not say. In the hub, when an approved one comes back presenting the pending one, the promotion in T1 is already the first confirmation, and the device becomes active there; the session only starts after the "saved" of the following credential. | C5 and C7. |
| **The order between proof and deadline.** 04 does not fix it. The hub looks at the proof of restoration before applying an expired deadline, so that the deadline does not hide the proof. | C7 and C11. |

To be proved in the MVP, by the suite: the swap with random drops, delays and repetitions; the late
response of an abandoned attempt; the revoking between the credential and the "saved"; the limits of
before the session; the message cut into chunks; and the local socket on both systems. The single
list is still in [12-roteiro.md](12-roteiro.md).

**Progress, 11/11/2026.** On the hub's side, the suite already has a test for the swap with random
drops on both sides (with and without a copy of the files), the late response of an abandoned
attempt, the revoking between the credential and the "saved", the limits of before the session and
the message cut into chunks. The tests run against a test connector, not against the project's
connector, which does not exist yet, and only on Windows. Everything is missing on the connector's
side, including the local socket.

**Known limits of the hub, 11/11/2026:**

- Before the headers, the TCP connections have no ceiling: only the 5-second deadline limits them.
- The hub of the tests did not have a real proxy in front of it; the reading of the forwarded
  address was tested with a trusted proxy on the machine itself.
- The registry of the connections and the check of the 24-hour swap have no index per device: each
  notice goes through all the connections. The cost was not measured beyond 500 connectors.
- The abandoned attempt is covered by a directed test and by the randomized rounds; the window
  between the last check and the start of T1 has no test that forces it.

## 13. Open points

The code chose a reading where the text diverges from it, or where the choice is not a clear
improvement. The two readings stay here, until the maintainer decides.

| Point | Reading of the text | Reading of the code |
|---|---|---|
| **Unknown one with the restored mode on.** | The table of section 6: an unknown one with a valid seal is proof of restoration, unless it is already listed. With the mode on, this would restart the mode. | With the mode on, the unknown one is listed and is not proof. By C11, only three things restart the mode, and the unknown one is not among them. It avoids restarting the mode at each device of the lost timeline. The cost: a second restoration by image, done with the mode on, is not noticed through a device the database does not know. A way out would be to use the start time of the mode to separate the two cases. |
| **Unknown one again after the purge.** | The list of unknown devices has a retention of 90 days ([15](15-mvp.md), section 2, rule 11). | After the purge, the same identity goes back to being "listed now" and, with the mode off, becomes proof again. It is the same open point of C11: what is missing is how to tell the leftover of the lost timeline from a new restoration. Today the purge does not exist in the hub yet. |
