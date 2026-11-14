# 12 — Roadmap

**Status: proposal.** The order of construction, with the done criterion of each phase. The order
may change; the done criterion of each phase may not.

## Summary

```
0 PLANNING            these documents                                       done
1 PROOFS              check the assumptions and prototype the channel       in the channel: inside the MVP
2 HUB FOUNDATION      database, accesses, engine under the hub's command    ┐
4 CONNECTION          connector, invite, code, sealed credential            ├ FIRST MVP, under construction
5 WAKE                notices from the hub to Hermes                        ┘
3 PANEL               login, numbers, matrix, audit
6 ARCHIVE             stored history, what is new and search
7 PUBLICATION         license, signed versions, installation guide
```

On 10/11/2026 phases 2, 4 and 5 were gathered into the **first MVP**, which also brings the
conversation between Hermes ([15-mvp.md](15-mvp.md), [decision 0033](decisoes/0033-escopo-do-mvp.md)).
The panel is left for after it: in the MVP, administration is done by command. The done criterion of
each phase still holds.
On 12/11/2026, at the maintainer's request, the panel started with the read-only screens and the guide
to connect Hermes ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)); the rest of Phase 3 is
still to be done.

"Publication" is opening the repository to the public. Until then the project has no version in use
by third parties.

## Phase 1 — Proofs

Before writing the project, clear up the doubts that may change the design. This is the **single
list** of what needs to be proved; the other documents point here.

### The environment of the proofs

It is not written yet, and it is the first delivery of the phase: how to bring up a Hermes Agent in
versions 0.22 and 0.23 for testing, and an engine with a test number. Without this, four of the
proofs below do not run.

### On Hermes

| What to prove | How |
|---|---|
| Hermes accepts the connector as a tool server | Register a minimal program through the official command, in versions 0.22 and 0.23, and restart Hermes. Also measure how long it takes to start the program without being restarted. |
| How the connector obtains the key of the local API | Test the first path of [11-conector.md](11-conector.md), section 6; if it does not work, the second ([decision 0023](decisoes/0023-chave-da-api-local.md)). |
| The block arrives identical through WhatsApp | Paste from the phone and from the computer and compare byte by byte on Hermes's side. |
| What Hermes does on receiving the connection sentence and reading the instruction page (not tried yet; the block alone was refused on 14/11/2026, cause unknown) | Whether it asks for human approval, whether it keeps the procedure in memory, how much time the terminal gives the command. |
| How many copies of the connector Hermes starts | Bring up all of Hermes's processes and count. |
| Hermes restarts a tool server that exits | Terminate the minimal program and see whether, and in how long, it comes back. |
| The call to the local API | Address, body, in which conversation the run happens and how to use the repetition identifier. |
| What changes in version 0.23 | Repeat each line of [hermes-fatos](../referencia/hermes-fatos.md). |

### On the channel

Since 10/11/2026 these proofs are done inside the MVP, as tests of the suite
([15-mvp.md](15-mvp.md), section 7). On 11/11/2026 the hub's side already has a test, against a
test connector and only on Windows, for: the handshake and the enrollment; the swap with randomized
drops on both sides; the 24-hour swap with the "saved" that does not arrive; the copy with the real
one on and off; the restored hub with the hub up and with the `admin` before the `serve`; and the
messages cut into pieces. None is "confirmed" yet: the project's connector is missing, and so are
the proofs that depend on it (lost file, change of folder, repeated commands, crossed leave and
reconnect).

| What to prove | How |
|---|---|
| The handshake and the enrollment | Two minimal programs, with the invite and the verification code. |
| The code cannot be chosen | On the side of whoever enrolls, try to reach a target code by redoing the enrollment. |
| The swap of the credential withstands a drop | Randomized drops on both sides, at any point of the swap, including the first one. |
| The swap withstands delay | Messages delivered late and out of order. |
| The hub's write order | The hub goes down between writing the pending one and sending the new one, and between hearing the "saved" and writing the promotion. |
| The 24-hour swap | The swap inside the open session, with a drop in the middle and with a "saved" that does not arrive. |
| The copy shows up | A connector that presents the credential and never confirms; a copy that connects with the real one off and with it on. |
| The lost file | The connector without the credential file, and with the old file after a simulated power failure. |
| The restored hub | The database coming back from an old backup: with devices that swapped credentials after it, with a device reconnected after it, with the backup taken without the hub restarting, and with the whole server coming back from an image. The drop between writing the database and writing the mark. |
| Reconnect with a drop | The machine goes down in the middle of the change of folder of the device. |
| Repeated commands | `join` and `leave` run again in each situation: pending, approved, expired, revoked, hub down. |
| Crossed leave and reconnect | `leave` in the middle of a reconnect; an invite of another line pasted into a Hermes already connected; `leave` done by whoever has a copy. |
| Large messages | How to split what goes over 64 KB. |

Written deliveries of the phase: the format of the messages of the channel, the content of the seal,
the function of the verification code, the check of rule 3 of the swap
([04](04-seguranca-conexao.md), C7), the handling of the credentials left over from a restoration
already handled (C11) and the format of the connector's local socket. It is also defined how the hub
checks the credential of a connector below the minimum version before refusing it: the rule is
decided ([decision 0030](decisoes/0030-conector-desatualizado.md)), the order of the messages is
missing ([04](04-seguranca-conexao.md), section 6, item 3). All of this is written, as a proposal,
in [14-protocolo-do-canal.md](14-protocolo-do-canal.md).

**Done when:** each assumption is marked "confirmed" or "does not hold, and the design changed"; and
the prototype goes through thousands of randomized rounds without locking out a legitimate device
that kept its files, without accepting a reused credential and without letting a copy taken outside
use the hub without this showing up when the real one presents its credential again.

The decisions that were blocking the code were taken on 10/11/2026: names in the code in English,
fixed send ceiling, inactivity of 30 days, the order of attempts for the key of the local API and
sending a file by public address ([decisions](decisoes/README.md)). The new name of the project was
applied to the documents on 10/11/2026 ([decision 0026](decisoes/0026-nome-do-projeto.md)).

## Phase 2 — Hub foundation

The code of the hub is written from scratch ([decision 0033](decisoes/0033-escopo-do-mvp.md)). The
source service is private and is not copied; the ideas that come from it are already in these
documents. The names in the code go in English ([decision 0016](decisoes/0016-idioma.md)).

What goes in: the database with organization, Hermes, number and access; the engine adapter as the
only point of contact; the hub as the owner of the configuration of the engine sessions; the tools
with the `number` argument; the authorization on every call; the send ceiling, with the value
fixed in this phase; the refusal of an address that leads to the internal network, with a test; and
the fake programs (engine and Hermes) for the tests.

**Done when:** there is an automatic test that a Hermes does not reach the number of another;
removing an access with the session open holds on the next call; the tests run against a fake
engine; the hub detects and fixes an engine session configured from outside.

## Phase 3 — Panel

First access, login by passkey, the screens of Hermes, WhatsApps, pairing, accesses and audit.

**Done when:** a test number is paired by code and by QR through the screen; no screen, log or
audit shows an invite, QR or credential outside the moment of use; a contact name with malicious
code appears as text; an action that changes something, without the passkey, is refused.

## Phase 4 — Connection

The connector with the four commands, the single leader, the invite, the typed code, the sealed
credential, the quarantine, revoke, reconnect and leave. The screens to connect and to confirm. The
inactivity period, with the label of the screen that is still missing
([decision 0021](decisoes/0021-prazo-de-inatividade.md)).

**Done when:**

- From zero to the first Hermes reading a number takes less than 15 minutes, following only what the
  screen says.
- Each failure of the catalog has its own message on both sides.
- Copying the connector's folder to another machine and using it leads to quarantine when the real
  one connects.
- Revoking brings down the channel in less than 5 seconds.
- There is a test for each line of the [threat model](05-modelo-de-ameacas.md) that has a testable
  rule.

## Phase 5 — Wake

The notice queue, the fixed sentence and the call to Hermes's local API.

**Done when:** with randomized drops of the hub, of the connector and of Hermes, no notice
disappears before expiring and no repeated notice starts a second run in Hermes; the notice does not
carry text written by third parties.

## Phase 6 — Archive

Importing the history the engine keeps, storing the whole message and the files on arrival, and the
tools for what is new and for search.

**Done when:** Hermes reads a chat with the engine off; the search finds by text, by person and by
period; the purge deletes what expired, including the files.

Until this phase, every reading depends on the engine being up.

## Phase 7 — Publication

License in the repository (Apache-2.0), with the rights holder defined, trademark registration of
the name checked, history of the repository
clean, README in English with the translations ([decision 0016](decisoes/0016-idioma.md)), signed
versions with the digest published, installation guide (with the panel closed by default), private
flaw reporting turned on in GitHub and the maintainer's page.

**Done when:** someone from outside installs the hub, connects a Hermes and pairs a number following
only the guide.

## After

| Item | Remark |
|---|---|
| Shielded mode | The connector separate from Hermes, installed by a person. |
| Second engine | Adapter for the official WhatsApp API. |
| Conversation between Hermes: rooms and observer | The note between two Hermes already goes into the first MVP ([13](13-conversa-entre-hermes.md)). |
| Other conversation platforms | E-mail, Telegram and others, on the same connection screen. |
| Fleet management | Creating, updating and monitoring the Hermes from the panel. It requires reviewing the rule that the connector executes nothing by order of the hub. |
| Several organizations in one hub | With one engine per organization. |

## How it is verified

The standard comes from the source service and holds for all phases:

- **Fake engine and fake Hermes** in the automatic tests, to run everything without a real WhatsApp.
- **Randomized rounds** in the queues and in the swap of the credential: the test drops, delays and
  repeats at random points and checks that nothing is lost nor repeated.
- **Mutants** in the security rules: the code is altered on purpose, and some test has to fail.
- **Canary:** a known secret is planted in the tests and cannot appear in any output, log or
  response.
- **Adversarial review** before each publication.
- **Contract test**, done by hand, against the real engine and the real Hermes, with a test number,
  on each new version of one of them. It is the only exception to the rule of not testing on a real
  number.
