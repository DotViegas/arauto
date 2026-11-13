# 15 — The first MVP

**Status: proposal under construction.** Revised on 10/11/2026, after the design review, and on
11/11/2026, with the hub code of stages 2 and 2b (what the hub does is marked "in the hub"; the
progress is in section 7, and the open points in section 9). What goes into the first version that
works end to end, with which technology and in which order. The decisions
are in [0031](decisoes/0031-banco-postgresql.md),
[0032](decisoes/0032-conversa-entre-hermes-no-mvp.md), [0033](decisoes/0033-escopo-do-mvp.md),
[0034](decisoes/0034-nomes-em-ingles.md), [0035](decisoes/0035-pilha-do-websocket.md) and
[0036](decisoes/0036-windows-so-para-desenvolvimento.md).

## Summary

```
 M1 CONNECTION    invite, typed code, sealed credential, encrypted channel, quarantine,
                  revoke, reconnect, leave, single leader, restored mode
 M2 CONVERSATION  notes between Hermes, peer list, ceilings, notice queue
 M3 WHATSAPP      engine (fake and WAHA), numbers, access matrix, tools
 M4 WAKE          the connector starts a run in Hermes from a notice
 M5 DELIVERY      container image, composition with PostgreSQL, full load test
```

| Question | Answer |
|---|---|
| What does the MVP do? | It connects Hermes to the hub, lets a Hermes send a note to another and use WhatsApp numbers, and wakes Hermes when something arrives. |
| How is it administered? | By a command on the hub's server. The panel started on 12/11/2026: read-only screens and, since 13/11/2026, the guide to connect Hermes, the editing of accesses and the revoke ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)). Pairing numbers and the passkey come later. |
| Where does it run in development? | Directly through `cargo`, with a local PostgreSQL. No container. |
| Where does it run in production? | In a container, on Linux, with PostgreSQL, through a compose file. |
| What is left out? | The passkey, pairing numbers through the panel, the message archive, the signing of versions and the shielded mode. |

## 1. What goes in and what does not

| Goes in | Does not go in, and is left for later |
|---|---|
| Invite, enrollment, typed code, approval | The passkey and pairing numbers through the panel ([07-telas.md](07-telas.md)). The read-only screens went in on 12/11/2026; the guide to connect Hermes, the editing of accesses and the revoke, on 13/11/2026. |
| Noise channel, swap of the credential, quarantine, revoke, reconnect, leave | Warning to the admin outside the panel ([decision 0025](decisoes/0025-aviso-por-fora-do-painel.md)) |
| Restored mode ([04](04-seguranca-conexao.md), C11), with the mark outside the database | The open point of C11 (the issue date of the seal) |
| Single leader and relays | Shielded mode |
| Conversation between Hermes ([13](13-conversa-entre-hermes.md)) | Observer, rooms and reading of the text by the admin |
| Fake engine and WAHA adapter; read, send and modify | Message archive and search (reading depends on the engine being up) |
| Access matrix, by number and by chat | Sending a file by address |
| Notices and waking, with the fixed sentence | List of versions of the program, with revocation and signature |
| Audit that only grows, and alerts read by command | Adjustable ceilings |
| Send ceiling (20 per minute, per Hermes and per number) and note ceilings | Private authority in the connector's TLS |

**Administration in the MVP.** Everything that changes something is
done by `arauto-hub admin`, run by a person **on the hub's machine**, or by the panel, which calls
the same functions and is only opened by a code that this command issues. Whoever runs this command
already has the database and the master key in hand; it does not create a weaker path. The hub and
the command are the same program and use the same write functions.

```
 arauto-hub init                              creates master key, mark, organization and hub key
 arauto-hub serve                             the hub
 arauto-hub migrate --i-have-a-backup         applies migrations to a database that already has data
 arauto-hub admin status                      situation, public key of the hub, restored mode
 arauto-hub admin hermes add|list|remove      the Hermes lines
 arauto-hub admin invite NAME                 issues the invite and shows the block (connect or reconnect)
 arauto-hub admin approve NAME [CODE]       the code is TYPED; without it, the command asks
 arauto-hub admin deny|revoke NAME
 arauto-hub admin wake NAME on|off            may be woken
 arauto-hub admin peers allow|deny|list       the peer list (--wake, --both)
 arauto-hub admin threads [NAME]              threads: who, when and how much; never the text
 arauto-hub admin engine|number|access ...    engine, numbers and access matrix (M3)
 arauto-hub admin alerts [--ack ID]           the alerts the panel shows
 arauto-hub admin audit [--tail N]
 arauto-hub admin panel code                  issues the entry code of the panel
```

**The panel, in the hub, on 12/11/2026** ([decision 0040](decisoes/0040-painel-dentro-do-hub.md), proposal). The hub serves the panel
on a third port, turned off by default. One enters with an entry code: the command above shows it
once, it is valid for 10 minutes and for one entry, and it is typed on the screen, never shown by
it. The panel has three read-only screens (Hermes, WhatsApps and Accesses), the alert banner and, since
13/11/2026, the guide to connect Hermes: name, block, typed code and approval, through the same
functions as `hermes add`, `invite`, `approve` and `deny`. On the same day the editing of the access
of a Hermes on a number (the `access grant`, the `wake-wait` and the `access remove`) and the
`revoke` went in. Pairing numbers and choosing the chats of an access remain only through the
command. By decision of the maintainer, the panel session acts without a passkey while it does not exist, and
for that reason the panel stays closed, reached through a tunnel. It was tested only by the
automatic tests; it never ran on a server and never connected a real Hermes.

- When approving a reconnect, the command shows the last use of the current device and requires
  `--recognized` or `--not-recognized` (04, C10). Without one of the two, it does not approve. In the
  hub, the command always prints the request, with the last use, before the result, and the approval
  repeats the last use. The question comes before the code: without an answer, nothing is counted,
  and "I do not recognize it" only holds together with the right code. It is also asked when the
  previous device expired by inactivity; revoked or replaced does not raise a question (section 9).
- The verification code stays in the clear on the device's row until the request is resolved, and is
  then erased. Passed as an argument, it stays in the terminal history: it is valid once and dies in
  10 minutes. A code outside the six-digit format does not count as an attempt.
- The actor recorded in the audit is "local admin", with the system user that ran the command, as
  declared text. In the other events, the actor is the device (enrollment, activation, confirmed swap
  and `leave`) or the system (quarantine, replacement and deadlines).

**In the hub, on 11/11/2026.** There are `init`, `serve`, `migrate`, `dev-reset` and, in `admin`,
`status`, `hermes`, `invite`, `approve`, `deny`, `revoke`, `wake`, `alerts` and `audit`. `peers` and
`threads` come in M2; `engine`, `number` and `access`, in M3. What the command does and the text did
not say:

- **`deny`** only denies a pending request. An approved one still without a credential leaves
  through `revoke` or through a new invite.
- **`hermes remove`** closes everything that belongs to the Hermes, like a revoke (device, request
  and invite), and removes its links. A deleted line with a device serving would be a Hermes nobody
  sees.
- **`alerts`** also lists the unknown devices of C11, with the time, the address and what it
  declares, escaped. `--all` shows the alerts already seen. `audit --tail` shows the last 50 by
  default.
- **With the restored mode on**, every `admin` command starts with the C11 warning.
- **With the database restored and the mode still off**, `admin` only refuses: it does not write,
  does not touch the mark and tells you to start `serve`, which turns the mode on. `status` keeps
  reading and exits with an error.
- **`status`** with more than one organization lists neither Hermes nor alerts: the commands that
  write require exactly one.
- **Durability.** Every command that writes checks `fsync` first, like `init`.
- **`init`** on a database with data and without a mark answers "already installed" and does not
  recreate the mark. On an empty database with a mark above zero, it refuses.

## 2. Database: PostgreSQL

The database becomes PostgreSQL ([decision 0031](decisoes/0031-banco-postgresql.md)). The model is
that of [06-modelo-de-dados.md](06-modelo-de-dados.md), with the names in English.

| In the model | In the database | Remark |
|---|---|---|
| organizacao | `organization` | |
| admin | `admin` | In the MVP, the "local admin". |
| hermes | `hermes` | The name follows the restricted format ([13](13-conversa-entre-hermes.md), section 2). |
| aparelho | `device` | `slot` (`in_use`, `request` or null) and `first_confirmed_at`. Unique index on (Hermes, `slot`). |
| aparelho_desconhecido | `unknown_device` | |
| convite | `invite` | Only the digest. |
| conexao_canal | `channel_connection` | One row per checked connection of a known device. |
| motor, numero, conversa | `engine`, `number`, `chat` | |
| acesso, acesso_conversa | `access`, `access_chat` | |
| evento_motor | `engine_event` | Inbox. |
| aviso | `notice` | Outbox, with `kind` (`whatsapp` or `peer`), the target of each type, `not_before` and `leased_until`. |
| auditoria | `audit` | Triggers forbid altering, deleting and truncating. |
| chamada_ferramenta | `tool_call` | Short retention. |
| instalacao, chave_hub | `installation`, `hub_key` | |
| (new) alert | `alert` | What the panel shows in the strip at the top. |
| (new) entry code, sessao_painel | `panel_entry`, `panel_session` | Only the digest of each one. [decision 0040](decisoes/0040-painel-dentro-do-hub.md). |
| (new) link | `peer_link` | [13](13-conversa-entre-hermes.md) |
| (new) thread, note | `peer_thread`, `peer_message` | The text of the note goes encrypted. |
| (new) retention | `retention` | Retention period per table. |

The `slot` of the device: `request` while it has not confirmed the first credential and the state is
not final; `in_use` after the first confirmation, in active, suspended or quarantine; null in a
final state. On a reconnect, the old one leaves the `slot` before the new one enters, in the same
write.

Rules that hold for the database:

1. **One database only.** The two files of the previous design existed because SQLite has one writer.
2. **The organization comes resolved.** Only a closed list of functions finds the organization from
   the outside: by the device identity, by the digest of the invite and by the engine's event
   secret. The system tasks that cross organizations (queues, deadlines, purge, mark) have a name
   and are also on the list. Every other function of the repository requires the organization
   already resolved. A test reads the repository code and fails if a function outside the list
   appears without this parameter. In the hub, the list has the entry by the identity and the entry
   by the invite (the engine's comes in M3), and the named tasks: installation, administration
   command, queues, deadlines, purge, mark, seal of an unknown device and start of `serve`. On
   12/11/2026 the two entries of the panel went in: by the digest of the entry code and by the
   digest of the identifier of the panel session. The only function that writes without an organization is that of the refusals of
   invites nobody issued. The test reads all the hub code that reaches the database, not only that
   of the repository, and each exception on the list says why it exists.
3. **Identifiers drawn in the hub** (UUID version 4). The only sequence is that of the audit.
   Migration 0002 replaced the integer identifier of `tool_call` with a UUID, to comply with this
   rule.
4. **Time belongs to the hub.** Time columns are integers, in UTC milliseconds. No column has a
   default time value and no SQL uses `now()`: a test sweeps the migrations. Every connection sets
   `lock_timeout` (5 s) and `idle_in_transaction_session_timeout` (30 s). The only exception is the
   control table the migrations library creates, `_sqlx_migrations`, whose `installed_on` column
   has the database time as default. No rule reads it, and the clock tests leave it out.
5. **The queues stay in the database.** Taking an item is a short `UPDATE`, over a selection with
   `FOR UPDATE SKIP LOCKED`, which marks the item as sent with a lease deadline, and is committed
   before the sending. An expired lease goes back to pending. No lock waits for the connector's
   confirmation. The worker only fetches notices of Hermes with an open session.
6. **A write that changes a device rereads first.** Approving, judging, promoting, replacing and
   revoking open a transaction, lock the row of the Hermes and then those of its devices, reread
   and only then write ([14](14-protocolo-do-canal.md), section 6). No transaction stays open
   during a network wait.
7. **Durability.** The security event transactions and those of the swap run with
   `SET LOCAL synchronous_commit = on`. At start, the hub checks `fsync` and `full_page_writes` and
   refuses to come up with them off, except with an explicit test option
   (`ARAUTO_ALLOW_UNSAFE_DB=1`). A lost commit of the swap would look like a restoration and would
   suspend everyone. The metadata of `tool_call` may go without waiting.
8. **Migrations embedded in the program**, forward only. `init` applies everything on an empty
   database. On a database with data, `serve` does not come up with a pending migration: whoever
   installs takes the backup and runs `migrate --i-have-a-backup`. `admin` refuses to run against a
   database of another version. In the hub, on 11/11/2026, there are two: 0001, of the core, and
   0002, of the connection review (pending proof of an unknown device, `tool_call` with UUID,
   keys composed with the organization, and indexes). The WhatsApp tables, which 0001 announced for
   0002, come in a later one. A development database created only with 0001 needs
   `migrate --i-have-a-backup` or `dev-reset`.
9. **The mark stays outside the database**, beside the master key (04, C11). A single function, used
   by the hub and by the admin command, takes care of it: under a file lock between
   processes, it checks that the database is not behind the mark before writing a security event,
   and then advances the mark. The mark only increases. Writes that wait are joined into one. The
   hub repeats the check at start and at every sweep: a database restored with the hub up is noticed
   in seconds, and an `admin` run before `serve` does not erase the proof. In the hub, the lock
   covers only the reading and the rewriting of the mark file, and not the check (section 9). A
   damaged mark is an error, and not "absent". "Database with data" means: there is a row in
   `installation`, or any row in `audit`.
10. **The audit only grows**, and keeps only what changes who may do what. The triggers protect
    against a program error; whoever has the database manages to delete. Repeated refusals and notes
    do not generate one event each ([13](13-conversa-entre-hermes.md), section 7). In the hub, each
    event says whether it is a security one, and the list is closed and has a test. Security events
    are: the installation, the organization, the hub key, the admins, creating and deleting Hermes,
    changing the "may be woken", invite used or cancelled, approving and denying, activating,
    quarantine, replacement, revocation, confirmed swap, the links of the peer list and the restored
    mode. Not security events: invite issued, expired, refused or used
    by another identity,
    repeated enrollment, wrong code, device expired by deadline, swap without "saved", the alerts of
    reuse and of a seal without explanation, connection taken over, note ceiling, unknown device
    listed and migration. Expiry by deadline stays out because the C11 list does not include it.
    From the panel, on 12/11/2026: the entry is a security event; the entry code issued and the
    exit are not.
11. **Purge.** A task of the hub deletes in small batches, with the hub's time, by the period of
    each table: notes, 30 days; tool calls, 7; resolved notices, 7; engine events already handled,
    1; connections and unknown devices, 90. On 11/11/2026 the periods are in the `retention` table,
    but the task does not exist yet. See also the second open point of
    [14](14-protocolo-do-canal.md), section 13. On 12/11/2026 the entry codes and the panel
    sessions went in: 30 days after the moment they stopped being valid.
12. **The database is accessed directly**, with no connection pooler in transaction mode. The hub
    has two connection pools of fixed size: that of the connectors (16 to start with) and one of 2,
    only for the sweep. Waiting for a connection has a deadline of 5 seconds. The sweep pool exists
    so that the revoke holds in under 5 seconds even with that of the connectors busy, for example
    in a mass return. The `NOTIFY` listener uses a third connection, outside the two, and the
    admin command opens its own pool, of 2. The panel uses the pool of the connectors.

**How the hub finds out.** What the hub itself enqueues wakes the worker by an in-memory signal.
PostgreSQL's `NOTIFY` serves only between processes: the admin command writes, and
notifies the hub with a short text, of the kind `revoked:{id}`. The hub listens on a connection of
its own, outside the pool. As this notification is not kept, the hub also **sweeps every 2 seconds**
and every time the listener reconnects: it rereads the state of the devices with an open connection,
the tool list of each session and the queues. It is the sweep, not the notification, that
guarantees the revoke in under 5 seconds.

In the hub, on 11/11/2026, the `NOTIFY` channel is `arauto_admin`, and the text is `type:id`, of a
device or of a Hermes: its connections reread the state at once. Each sweep, in this order: checks
the mark (a restored database turns the restored mode on); applies the deadlines of each
organization; rereads the state of every device with a connection, in one query per organization,
closes with the code of the state the session that no longer serves and nudges the waiting
connection whose request left pending; writes the enrollment refusals of the minutes that closed;
and, only with the mode on, checks whether it has ended. There is not yet a tool list that changes
nor a notice queue (M2). An expired deadline is also applied at once by whoever finds it before the
sweep: the enrollment, the approval, T1 and T2, which also apply the 30 days of inactivity. At
start, `serve` closes the connections the previous run left open.

A second hub serving the same organization is not handled in the MVP: the connection registry lives
in the memory of one process.

## 3. Websocket: how performance is sought

The choice is in [decision 0035](decisoes/0035-pilha-do-websocket.md). What weighs on the hub is not
the websocket: it is the cipher of each frame and the database.

| Care | How |
|---|---|
| Server | `axum` over `hyper`, on `tokio` with several cores. The loop that accepts connections belongs to the project: it has a deadline for the headers, a connection ceiling and a cancellation signal per connection. In the hub, the loop and the routes belong to the project, over the HTTP/1 of `hyper`; from `axum` come the websocket and the HTTP types. The ceiling of connections without a session is checked before opening the websocket. |
| One connection, two tasks | One reads, the other writes. Only the one that writes encrypts ([14](14-protocolo-do-canal.md), section 3). |
| Two output queues | Control (16 frames) and data (64 frames). The task that writes looks at control before each frame. |
| Backpressure | Whoever produces data waits for room in the queue. The connection is only closed when a frame stays 30 seconds without going out, or when the control queue fills up. A slow network expels nobody. Whoever sends data cannot give up in the middle of a message: the stream would be left incomplete and the connector would close the connection by the stalled stream deadline. |
| Cipher without a lock | Noise is used in the mode in which the counter is passed by hand, one per direction. The refusal of a repeated or out-of-order frame is project code, with a test. |
| Memory | Limits per phase and a budget of the whole hub for messages being reassembled ([14](14-protocolo-do-canal.md), section 4). A message above 256 KiB is parsed outside the network tasks. |
| No compression | The content already goes encrypted. |
| No waiting on send | `TCP_NODELAY` on in each accepted connection. |
| Connection table | A sharded map, by device identity. |
| Database | Pools of fixed size, one for the connectors and one for the sweep; short queries, by index. |
| Reassembly deadlines | Measured on a clock that only moves forward, and not on the hub's clock: a jump of the hub's clock does not cut a message that is arriving. |
| Authorization on every call | One query by index, with no cache. It is rule 9 of the project; one measures before optimizing. |
| Mass return | The ceiling of connections without a session holds the first wave; whoever is refused comes back with a drawn wait. The writes of the mark are joined. |
| Orderly stop | The hub stops accepting, sends `closing` to the connections without a session, lets each session answer the calls in progress and close, waits up to 8 seconds, closes by force what is left and exits ([14](14-protocolo-do-canal.md), section 7). The signal is SIGTERM, or Ctrl+C in development. |

**Target and load test.** A "local tool" is one the hub answers without talking to the engine.

| Item | Scenario and criterion |
|---|---|
| Load | 2,000 simulated connectors, in another process, each with its own identity. |
| Steady state | Heartbeat; one local read call every 10 seconds per connector; one in ten sends a note per minute. |
| Mass return | The hub restarts and all reconnect. |
| Approved when | Local read: 95% within 20 ms and 99% within 100 ms. Note: 95% within 50 ms. The 2,000 sessions back within 60 seconds, with no quarantine. Up to 100 KiB of memory per idle session. |

They are targets: they only become data when the test runs, and the result holds for the machine it
ran on. M1 already runs a smaller version (500 connectors: mass return, heartbeat and local read).

**The smaller version, run once on 11/11/2026.** On a development computer with Windows, in a
development build, with the hub and the 500 test connectors in the same process, all coming from
127.0.0.1 (so the ceiling of 16 connections without a session per origin held for all), a pool of 16
connections and a `ping` every second. It is marked not to run with the normal suite, and it was not
repeated after the last round of channel fixes, which added the sweep pool.

| Measure | Result |
|---|---|
| Preparing 500 devices (enrollment, approval, session) | 9.1 s |
| Mass return | 500 sessions in 5.4 s; 648 attempts refused (429 or AR-205) and repeated; no failure |
| `arauto_status`, 5,000 calls | p50 7.1 ms; p95 30.5 ms; p99 42.1 ms; maximum 72.0 ms; no failure |
| Quarantines | None |

The p95 stayed above the target of 20 ms; the p99 and the mass return, within. The scenario is not
that of the target: 2,000 connectors in another process, in a production build, were not measured,
nor the memory per idle session.

## 4. The code

A workspace in Rust, with the `Cargo.lock` versioned.

```
crates/
  arauto-protocol/    what the two programs share: frames, messages, Noise,
                      invite, verification code, name format, limits and deadlines,
                      error catalog and tool catalog
  arauto-platform/    what changes from one system to another: durable write, file
                      lock, local socket, stop signals
  arauto-hub/         the hub: server and admin command
  arauto-connector/   the connector
  arauto-testkit/     for tests only: fake engine, fake Hermes, load generator
deploy/               Dockerfile and compose file
```

Modules of the hub ([03-arquitetura.md](03-arquitetura.md), section 3): `enrollment`, `channel`, `vault`,
`core`, `engine`, and the new `peers`. `archive` does not exist in the MVP; `panel` went in on 12/11/2026, with the read-only screens, and gained the guide to connect Hermes, the editing of accesses and the revoke on 13/11/2026. In the code, on
11/11/2026: the enrollment lives inside `channel`, and there are also `repo` (the repositories), `audit`,
`mark`, `restore`, `admin`, `config` and `db`. `engine` and `peers` come in M3 and in M2.

| Piece | Library |
|---|---|
| Server and websocket of the hub | `axum`, `hyper`, `hyper-util`, `tokio` |
| Panel | Pages written by the hub itself, with no template library; one script file of the panel itself, and no script inside a page ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)) |
| Websocket of the connector | `tokio-tungstenite`, with `rustls` and the public authorities embedded |
| Database | `sqlx` (PostgreSQL), queries at run time |
| Noise | `snow` |
| Seal and encryption at rest | `chacha20poly1305` |
| Digest and derivation | `sha2`, `hkdf` |
| Random draw | `getrandom` (system generator) |
| Secret in memory | `zeroize`; comparison with `subtle` |
| Messages | `serde`, `serde_json`; base32 and base64 with `data-encoding` |
| MCP server over stdio | Written in the project: it is small and it is the text the LLM reads |
| Local socket and lock | `tokio` and the standard library, inside `arauto-platform` |
| Client of the engine and of the local API | `reqwest` |
| Command line and logging | `clap`, `tracing` |

The minimum Rust version is the one pinned in the repository. In production the connector is a
static binary for Linux x86_64.

**Configuration**, by environment variables:

| Program | Variable | What for |
|---|---|---|
| hub | `ARAUTO_DATABASE_URL` | The database. |
| hub | `ARAUTO_DATA_DIR` | Master key and mark. Outside the database volume. |
| hub | `ARAUTO_LISTEN`, `ARAUTO_LISTEN_INTERNAL` | The port of the connectors; the internal port (health and engine events). |
| hub | `ARAUTO_LISTEN_PANEL`, `ARAUTO_PANEL_URL` | The port of the panel and the address the admin opens in the browser. Without the two, the panel stays off; one without the other, `serve` does not come up. |
| hub | `ARAUTO_PUBLIC_URL` | The address that goes in the block: `https://host[:port]`, with no path, or `http://` only for 127.0.0.1, `[::1]` and `localhost`. |
| hub | `ARAUTO_CLIENT_IP_SOURCE`, `ARAUTO_TRUSTED_PROXIES` | Where the connector's address comes from: `socket`, or `forwarded` with the list of trusted proxies. Mandatory: without it `serve` does not come up. |
| hub | `ARAUTO_ALLOW_UNSAFE_DB` | `1` lets the hub come up on a database with `fsync` off. For tests only. |

**The connector's address, in the hub.** With `socket`, the address of the connection holds, and any
forwarding header is ignored. With `forwarded`, the hub only reads `X-Forwarded-For`, and only when
the connection comes from a proxy on the list (addresses or networks, up to 64). The list in the
header is read from right to left, up to 32 addresses: the first that is not a trusted proxy is the
connector, and what is to its left was written by the connector itself. A trusted proxy that does
not say who the connector is (without the header, with it invalid or with only proxy addresses) gets
400: taking the proxy's address would put all the connectors behind it in a single origin, and one
would fill the ceiling of all.
| connector | `ARAUTO_HOME` | The connector's folder. Default: `~/.arauto`. |
| connector | `ARAUTO_HERMES_BIN` | The Hermes command used in the registration. Default: `hermes`. |

When the folder is not the default one, the registration in Hermes carries `--home` in the
arguments: Hermes starts the connector with a filtered environment, and the variable would not
arrive.

## 5. How to run in development

No container. What is needed: Rust at the pinned version and a local PostgreSQL 16.

```
 1 start the local PostgreSQL and create the development database
 2 cargo run -p arauto-hub -- init            does not redo what already exists
 3 cargo run -p arauto-hub -- serve           the hub
 4 cargo run -p arauto-hub -- admin ...       creates Hermes, issues invite, approves, links the peers
 5 cargo run -p arauto-connector -- join ...  one folder per test Hermes, with ARAUTO_HOME
 6 cargo test --workspace --locked            the suite
```

- **Two connectors on the same machine:** each with its own `ARAUTO_HOME`. The folder defines the
  local socket and the leader's lock.
- **Without a real Hermes:** `arauto-testkit` brings a fake Hermes, which accepts the registration,
  starts the connector over stdio in as many copies as asked and has the local API that wakes.
  `ARAUTO_HERMES_BIN` points to it. `join --no-register` skips the registration.
- **Resetting the environment:** `arauto-hub dev-reset` erases the development database **and** the
  mark, together. Erasing only one of the two would turn the restored mode on, which is the right
  behavior. In the hub, it asks for `--yes-erase-everything`, only runs on a database whose name
  ends in `_dev` (or on a database of the suite), erases the `public` schema and not the database,
  and leaves the master key. It erases the database first: a stop between the two leaves the mark
  ahead of an empty database, which `init` refuses and a second `dev-reset` solves.
- **Windows** is a development platform for the hub and the connector
  ([decision 0036](decisoes/0036-windows-so-para-desenvolvimento.md)). What changes:

| Piece | On Linux | On development Windows |
|---|---|---|
| Durable write | Temporary file, forced write of the file, rename, forced write of the folder | The same, without the write of the folder |
| Replacing the program in use | Rename over it | Renames first the one in use |
| Local socket | Domain socket, owner only, with a check of the user | Named pipe, first instance, no remote client |
| Leader's lock | File lock | File lock, on an empty file only for this |
| Owner-only folder | Mode 0700 | The permission inherited from the user's profile |
| Stop | SIGTERM | Ctrl+C |

**The suite.**

- The tests with a database use the address in `ARAUTO_TEST_DATABASE_URL`. Without it they **fail**,
  with a message that says what is missing: they are not skipped in silence.
- The migrations run once, on a template database. Each test that needs its own database creates
  its own from the template; the others share a database and isolate themselves by organization.
  The user of the test database needs to be able to create a database. Old test databases are
  erased at start. In the hub, the databases of each test stay until the next run, instead of being
  erased at the end of the test. The tests of deadlines in seconds stay in a test binary of their
  own, because creating databases from the template locks the server for seconds.
- **The test kit** (`arauto-testkit`, in M1) brings a high-level test connector, with failures on
  command (dropping at each step of the swap, holding the `check_ok`, presenting another credential,
  not sending the "saved", copying the files), a hostile connector, with one method per abuse of the
  limits, and the generator of the mass return and of the measured reads. The fake engine and the
  fake Hermes come later.
- **The secret canary** runs a whole flow (invite refused and valid, wrong code, session, 24-hour
  swap, leave, copy in quarantine) and looks for each secret, in several encodings, in the process
  log and in the tables of audit, alerts, unknown devices, connections and invites.
- Each test has its own temporary folder of master key and mark, and a pool of 2 or 3 connections.
- **The swap rule is a pure function**: the state of the device plus an event give the decision and
  the writes. It is the one that goes through tens of thousands of randomized rounds, in memory,
  with simulated clock and network. The seed comes out on failure. A few hundred rounds against the
  real PostgreSQL check that the SQL follows the same model.
- Every deadline comes from configuration and from a clock the test controls.

## 6. How to deliver in production

In a container ([decision 0006](decisoes/0006-instalado-no-servidor-de-cada-um.md)).

| Service | What it is |
|---|---|
| `hub` | The project's image. It publishes the port of the connectors and that of the panel, both only on `127.0.0.1` of the server. |
| `postgres` | PostgreSQL 16, with its own volume, which needs to honor the forced write. |
| proxy | From whoever installs. It terminates the TLS of the port of the connectors. |
| engine | From whoever installs, referenced by address. **It goes neither in the image nor in the composition.** |

- The master key and the mark stay in a volume **separate** from the database volume and outside its
  backup.
- **Backup:** that of PostgreSQL, continuous (archiving of the transaction log) or, at the least,
  daily. Restoring turns the restored mode on, and all Hermes reconnect (C11).
- **The proxy needs to:** pass on the upgrade to websocket, have an inactivity deadline above 90
  seconds and not hold data in memory. The hub is configured with the list of trusted proxies to
  read the address of each connector.
- **The engine events** arrive through the internal port, which is not published: the engine
  reaches the hub through a shared network of the composition, and each event brings the event
  secret.
- The composition declares the hub's limit of open files and the stop deadline.
- The image brings the connector for Linux x86_64, served at the download address, with the SHA-256
  digest computed at build time.

## 7. Milestones and how each one is verified

The done criterion of each phase of the [roadmap](12-roteiro.md) still holds. The MVP regroups them.

| Milestone | Goes in | Done when |
|---|---|---|
| **M1 Connection** | Frames, streams and limits; handshake; enrollment and code; approving by command; the swap; session, heartbeat and 24-hour swap; quarantine, revoke, reconnect and leave; the deadlines (invite, pending, approved, inactivity); audit, mark and restored mode; alerts; leader, relays and local socket; MCP server with `arauto_status`; the `status` command; fake Hermes; reduced load. | A connector enters with invite and code. The swap goes through the randomized rounds without locking out a legitimate device, without accepting a reused credential and without letting a copy go unnoticed when the real one comes back. These have a test: the late answer of an abandoned attempt; revoking between the credential and the "saved"; the limits of before the session; revoking in under 5 seconds with the notification lost; database restored with the hub up; `admin` before `serve` after restoring. |
| **M2 Conversation** | Peer list; threads and notes; the ceilings; the four tools; the notice queue, with lease, grouping and re-evaluation; `notice` and `notice_ack` against a test destination; the tool list that changes with the peer list. | These have a test: without a link it does not send, and the answer is the same as for a nonexistent name; it neither reads nor lists a thread of others; it does not reply without the link back; removing the link cuts off the unread; the loop without `thread` stops at the ceiling; each ceiling; the notice carries no text; the connector refuses a `from` outside the format. |
| **M3 WhatsApp** | The engine interface, the fake engine and the WAHA adapter; numbers; access matrix; the WhatsApp tools; send ceiling; inbox of events; message notices. | These have a test: a Hermes does not reach the number of another; malicious `to` and `message_id` are refused; removing an access holds on the next call. All against the fake engine. |
| **M4 Wake** | The call to Hermes's local API; the memory of handled notices; the connector's ceiling of wakes. | With randomized drops of the hub and of the connector, no notice disappears before expiring and no repeated notice starts a second run. |
| **M5 Delivery** | Dockerfile, composition, download address, installation guide and the full load test. | The image comes up with the composition, the hub passes the health check and the load test meets the criteria of section 3, or the result is recorded with what was missing. |

The verification rules of the roadmap hold: fake engine and fake Hermes, randomized rounds, mutants
in the security rules and the secret canary.

**Progress of M1, 11/11/2026.** The hub side is written and has tests, on Windows:

| Stage | What went in |
|---|---|
| 1 | The common protocol and what changes from one system to another. |
| 2 | The database and the migrations; the vault; the mark and the restored mode; the swap rule as a pure function, with randomized rounds; the repositories; the admin command. |
| 2b | The channel in the hub: the loop that accepts connections, the handshake, the frames and the limits, the enrollment, the waiting, the swap, the session with `arauto_status`, the 24-hour swap, the sweep, the `NOTIFY` listener and the orderly stop. The test kit: test connector, hostile connector and load generator. |

Missing, for M1: the connector (with the leader, the relays, the local socket and the MCP server),
the fake Hermes, the purge and the reduced load against the real connector. Of the items of the done
criterion, the hub already has a test for: the late answer of an abandoned attempt; revoking between
the credential and the "saved"; the limits of before the session; revoking in under 5 seconds
without the notification, including with the pool of the connectors busy; database restored with the
hub up; and `admin` before `serve` after restoring. The randomized rounds of the channel (120 per
run, with the seed printed) drop the connection from each side at each step of the swap and use a
copy of the files in a third of them.

## 8. What the MVP does not prove

- **The proofs "in Hermes".** The registration by the official command, how many copies of the
  connector it starts, whether it updates the tool list when it changes, and the key and the format
  of the local API depend on a real Hermes. Until then the connector follows what is in
  [hermes-fatos](../referencia/hermes-fatos.md), and "being woken" is only taken as working against
  the fake Hermes.
- **The Linux path.** While development is only on Windows, the Linux-specific code is checked by
  the compiler and never executed. It needs to run on Linux, with the whole suite, before any real
  use.
- **The container.** The image and the composition are written in M5; they only hold after being
  built and tested on a server.
- **The real engine.** The WAHA adapter is tested against the behavior recorded in
  [waha-fatos](../referencia/waha-fatos.md). There is no automatic test on a real number.
- **The `NOTIFY` listener reconnecting** after a drop of the database. It was only read in the
  library's code; no test brings the database down. The revoke does not depend on it, but on the
  sweep.
- **A real proxy** in front of the hub, and the full load in another process.

## 9. Open points

Choices of the hub code that contradict this document, or that are not a clear improvement. Both
readings stay until the maintainer decides. The points of the channel are in
[14](14-protocolo-do-canal.md), section 13, and those that ask for a change in 04, in section 12 of 14.

| Point | Reading of the text | Reading of the code |
|---|---|---|
| **The panel entry without a passkey** ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)). | [04](04-seguranca-conexao.md) and [decision 0019](decisoes/0019-painel-com-passkey.md): the panel login is by passkey, and the first admin is born from the installation code. | The passkey does not exist yet. One enters with a single-use entry code, issued by a command on the hub's machine, and every panel session belongs to the "local admin". Since 13/11/2026 this session also connects Hermes, without a passkey: it is the change of 12/11/2026 in decision 0019, at the maintainer's request, with the panel closed while it lasts. |
| **The check of the mark outside the lock** (section 2, rule 9). | "Under a file lock between processes, it checks [...] before writing a security event, and then advances the mark." | The lock covers only the reading and the rewriting of the mark file. The check reads the file without a lock: it is replaced whole, and any value that has already been on disk serves the comparison. Holding the lock during the database transaction would put every security event in a queue and would prevent joining the writes. The review found no bad interleaving; either the text comes to say this, or the code comes to hold the lock. |
| **`--recognized` typed before seeing the last use** (section 1). | The command shows the last use and requires `--recognized` or `--not-recognized`. | The command always prints the last use before the result, but whoever types `--recognized` out of habit writes the option before seeing the screen. Closing this asks for a change to the command line, for example requiring along with it the date of the last use shown. |
| **Events without an organization in `audit --tail`.** | Every read is filtered by the organization (section 2, rule 2). | The read of one organization also brings the events without an organization: the start of the restored mode, which may have as its target a device of another organization and counts devices of all of them, and the aggregated refusals of invites nobody issued. With a single organization, as in the MVP, nothing leaks. With two, the admin of one would read the identifier and counts of the other. The way out would be a command of its own for the events of the installation, or removing from the event the target and the counts of other organizations. |
| **Two starts at the same time with the mark absent.** | Turning the mode on is a single write. | Two simultaneous entries may turn the mode on twice. The second suspends nobody new and only generates one more event. |
