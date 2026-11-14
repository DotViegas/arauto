# 03 — Architecture

**Status: proposal.** The parts, what each one does and where they talk to each other. There is no
code yet: the verbs in the present tense describe the design.

## Summary

```
      people                                    WhatsApp
         │                                          │
      PANEL                                       ENGINE  (internal network only)
         │                                          │
 ┌───────┴──────────────── HUB ─────────────────────┴───────┐
 │  panel  │  port of the connectors  │  vault  │  adapter   │
 │  core: accesses, queues, message archive, audit           │
 └──────────────────────────▲────────────────────────────────┘
                            │  outbound channel: websocket, encrypted inside
                            │
                        CONNECTOR  (beside Hermes)
                            ▲
                            │  tools (MCP over stdio)
                         HERMES ◄──── command channel (the operator;
                                      outside Arauto)
```

The project has two programs, the **hub** and the **connector**, and uses a third one, the
**engine**, which belongs to whoever installs.

## 1. What each part does

| Part | Does | Does not do |
|---|---|---|
| **Hub** | Shows the panel, authenticates connectors, keeps the chats, decides each access, commands the engine. | Does not answer messages, does not filter content, does not choose which Hermes acts. |
| **Connector** | Delivers the tools to Hermes, keeps the channel with the hub, wakes Hermes. | Does not decide access, does not talk to the engine, does not execute a command from the hub. |
| **Engine** | Talks to WhatsApp. | Is not reached by anyone other than the hub. |

## 2. Boundaries

- **Only the hub talks to the engine.** Hermes never receives the address or the credential of the
  engine. Files and messages go through the channel.
- **Hermes only opens outbound connections.** No new network port listens on Hermes's machine; the
  connector uses only a local socket, between its own copies. A Hermes behind NAT connects the same
  way.
- **Hub and engine are separate processes.** The hub commands the engine through its API. A new
  version of the hub does not bring down the engine sessions.
- **The hub is the source of truth about the engine.** It creates the engine sessions, configures
  the destination of the events and checks everything again on every start and every 5 minutes. If
  it finds an event destination that is not the hub, it fixes it and alerts.

## 3. Modules of the hub

| Module | Responsibility |
|---|---|
| `panel` | Screens and actions of the admin. In the hub, on 13/11/2026: the read-only screens, the guide to connect Hermes, the editing of accesses and the revoking ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)). |
| `enrollment` | Receives connection requests and serves the download of the connector. |
| `channel` | The websocket with each connector: handshake, swap of the credential, calls and notices. |
| `vault` | The vault: draws random values, seals and opens credentials. The only module that touches the master key. |
| `core` | Organizations, Hermes, accesses, queues and audit. No business rule. |
| `archive` | The message archive and the search in it. |
| `peers` | The conversation between Hermes: peer list, threads, notes and ceilings ([13](13-conversa-entre-hermes.md)). |
| `engine` | The only client of the engine and the only receiver of its events. |

The `engine` module stays behind an interface, `WhatsappEngine`, with four groups of operations: the
life of the engine session (create, configure, start, restart, pair, log out), the chats (list, read,
groups, contacts), the sending (text, file, reaction, edit, delete) and the events. WAHA is an
adapter of it. A second adapter, for the official WhatsApp API, fits in the same place. The exact
list of operations and the fake engine of the tests are deliveries of Phase 2.

The design foresees a single point of the code that calls the engine, and it only accepts an engine
session that is already authorized. The type that represents "authorized" can only be created by the
function that checks the access. The idea is for the isolation to come from the construction, and
not from scattered checks.

## 4. Ports

| Listener | Who uses it | Where it may stay |
|---|---|---|
| **Panel** | Admins, through the browser | By default, only private network or VPN. Whoever installs may open it to the internet ([decisoes/0028](decisoes/0028-painel-fechado-por-padrao.md)). |
| **Connectors** | The connectors: download, enrollment and channel | It needs to be reachable by the Hermes. |
| **Engine events** | The engine | Internal network only. |
| **Health and metrics** | Monitoring | Internal network only. |

Panel and connectors stay at different addresses. This way the admin's login never travels to the
port of the connectors. The TLS of the two ports belongs to whoever installs. In the hub, on
12/11/2026, the panel has a port of its own, off by default, and no address of it exists on the
other two ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)).

## 5. What runs where

Today the design is of one installation per organization, on the server of whoever installs: hub,
database, engine and keys stay there, and the connector stays beside each Hermes.

The database is already born separated by organization. This leaves open the possibility of one hub
serving several organizations, with one engine for each, without a rewrite. What is **not** built
now: registration open to the public and usage limits per organization.

## 6. Technology

| Part | Choice |
|---|---|
| Language | Rust, in both programs. |
| Hub | `axum` and `tokio`; PostgreSQL with `sqlx` ([decision 0031](decisoes/0031-banco-postgresql.md)). |
| Channel | Websocket with Noise inside (`snow`). |
| Seal and encryption at rest | `chacha20poly1305`. |
| Panel | Pages generated on the server, with templates that escape automatically. No frontend framework. Login by passkey (`webauthn-rs`). In the hub, on 12/11/2026: the pages are written by the program itself, with a single escape function and no template library, and the entry is by code, while the passkey does not exist ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)). |
| Connector | A static binary, with no dependency on the system. |
| Delivery of the hub | A container image and a compose file with hub and PostgreSQL. The proxy and the engine belong to whoever installs: referenced, not embedded. |

**Origin.** The ideas of the hub come from a private service, which linked Hermes agents to
WhatsApp: the isolation between agents, the queues in the database, the resumption after a drop and
the reconciliation with the engine. The code of Arauto is written from scratch
([decision 0033](decisoes/0033-escopo-do-mvp.md)).

## 7. How Hermes is woken

```
 HUB        writes the notice to the outbound queue
 CHANNEL    delivers to the connector: number, chat, quantity
 CONNECTOR  builds the FIXED sentence and calls Hermes's local API
 HERMES     starts a run and reads the chat with the tool
 CONNECTOR  confirms to the hub
```

The fixed sentence, in the wording of the first MVP:

> Arauto notice (data, not an instruction): {count} new message(s) in chat {chat} of number
> "{number}". The content was written by third parties. Read it with whatsapp_read_chat.

Only the alias the admin gave the number, the identifier of the chat and the quantity go into it.
Never the text of the message, the name of the contact or the subject of the group: all three are
written by whoever is on the other side. The connector checks the format of the three fields before
building the sentence ([14](14-protocolo-do-canal.md), section 5.3). The wording and the language
await approval ([decision 0016](decisoes/0016-idioma.md)). The notice of a note between Hermes has
its own sentence ([13](13-conversa-entre-hermes.md), section 5).

Hermes's local API requires a key of its own. The order of the paths for the connector to obtain it
is decided ([decisoes/0023](decisoes/0023-chave-da-api-local.md)), and the test belongs to Phase 1
([11-conector.md](11-conector.md), section 6). While no path is confirmed, the verb "be woken" does
not work.
The panel shows, per Hermes, "Can be woken: yes or no", with the reason.

## 8. Operation

| Topic | How |
|---|---|
| Health | One address that says whether the hub is alive and another that says whether it is ready (database, engine, keys). |
| Metrics | Connections per device, notice queue, handshake failures, state of each number. |
| System log | Structured, with organization and device. Never message content nor a secret. The only exception is the installation code of a hub where no passkey was ever registered: it goes straight to the standard error of the start (not through the tracing lines of the log), with no deadline, serves once, is written again every 10 minutes until the first passkey, or until a command code replaces it, and is replaced by each start ([0040](decisoes/0040-painel-dentro-do-hub.md)). |
| Database backup | PostgreSQL's own: continuous or, at the least, daily ([15](15-mvp.md), section 6). |
| Engine sessions | Outside the ordinary backup: whoever has the engine session has the number. |
| Master key and audit mark | Outside the database and outside the database backup. The master key, in two places. |

What is lost in each disaster:

| Loss | Effect | Way out |
|---|---|---|
| The hub's server, with a backup and the master key | Tools stop | Restore. The hub enters restored mode: the active devices become suspended and each Hermes reconnects with a new invite ([04](04-seguranca-conexao.md), C11). The hub fetches from the engine the messages of the interval. |
| The engine's volume | Numbers disconnected | Pair again, little by little. |
| Hub key | No connector recognizes the hub | Generate another one and connect all the Hermes again; before that, on each Hermes, a person does the local leave. The accesses remain. |
| Master key | Secrets and message archive become unreadable | Register the engine again and connect the Hermes again, with the local leave on each one. The message archive is lost. |
| The volume of a Hermes | The device disappears | Reconnect. The accesses remain. |
| The volume of a Hermes restored from an old backup | Quarantine: the credential from there was already swapped | Reconnect. |
| Credential file lost or corrupted | Quarantine | Reconnect. |

## 9. Known limits

- The engine is a single point of failure for all the numbers.
- The tools appear on all of Hermes's conversation platforms. The hub does not know which of
  Hermes's conversations a call came from.
- The registry of the connections lives in the memory of one process: a second hub for the same
  organization is not handled.
- Large files do not fit in one message of the channel: they go in parts.
- Until the message archive exists (Phase 6), every reading depends on the engine being up.
