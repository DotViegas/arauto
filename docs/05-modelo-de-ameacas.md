# 05 — Threat model

**Status: proposal**, following the design of [04-seguranca-conexao.md](04-seguranca-conexao.md).
What we protect, from whom, what each attack yields and what is left after the defenses.

## Summary

```
WHAT THE DESIGN HANDLES             WHAT IS LEFT (the three roots)
leaked chat or block                admin account taken over
copied credential                   hub server broken into
hub database read                   WhatsApp engine exposed
whoever terminates the channel TLS
one Hermes trying to reach another
```

**Where this model comes from.** Before any code existed, the design went through several rounds of
automatic review, done by AI agents that had only the documents. In the first, four attacked a
draft, each from one starting point (the leaked chat, the manipulated Hermes, the network and the
panel, the supply chain), and a fifth tried to refute the 51 findings: 34 held, in whole or in
part, and none was classified as critical. In the second, four readers with no context reread the
documents, already corrected, and found logic flaws in the credential swap, corrected in proposal
v0.3. The following rounds, with one to four readers each, found flaws in the hub restored from a
backup, in reconnecting, in leaving and in the verification code, corrected in proposal v0.4. The
full reports are not published. Review by people, from outside, has not happened yet.

## 1. What we protect

| Asset | Why it matters |
|---|---|
| The engine sessions | Whoever has them **is** the number: reads and sends everything. |
| The message archive | They are chats of third parties. |
| The admin account | It creates invites, approves devices and grants accesses. |
| The master key and the hub key | Sealing credentials and being recognized as the hub. |
| The identity and the credential of each Hermes | Acting as that Hermes. |
| The connector program | It runs inside each Hermes. |

## 2. From whom

| Who | What they can do from the start |
|---|---|
| Whoever reads the command channel | The block, before, during or after its use. |
| A WhatsApp contact | Writing what Hermes will read. |
| Hermes's LLM, manipulated | Running commands and reading files as Hermes's user. |
| Someone on the network | Seeing, delaying and altering the traffic between Hermes and hub, including at the proxy that terminates the TLS. |
| Someone with a stolen panel session | Seeing the panel. They do not have the passkey. **In the hub, on 13/11/2026, the passkey does not exist yet: this person also connects, reconnects and revokes Hermes and changes accesses, while the session is valid** ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)). |
| Whoever reads the database or the backup of the hub | Everything that is stored there. |
| Whoever breaks into the hub server | Everything the hub can do. |

## 3. What each leak gives

| What leaked | What the intruder gets | Why |
|---|---|---|
| Chat or block, after use | Nothing. | The invite was already spent and the verification code is not a secret. |
| Chat or block, before use | Disrupting the connection. Access, none. | The code the real Hermes said is missing. |
| Connector files, copied with Hermes on | The copy is stopped in seconds and the device goes to quarantine. If whoever copied also holds Hermes's network, they use the hub for as long as they hold it. | It is a double connection. The panel alerts when an open connection is taken by another; whoever waits for the connection to drop before coming in only shows up as a return from another address. |
| Connector files, copied with Hermes off | Using Hermes **until it comes back**. On its return, quarantine. A Hermes with the connector below the minimum version counts as off: the copy works until the admin reconnects, and its use may not show up as a copy. | To use it, the copy has to swap the credential, and the real one's dies. |
| Old copy of the connector files | Access, none, after the first swap that came after it. But presenting it takes Hermes offline. | Reuse leads to quarantine. |
| Traffic between connector and hub | Nothing. | The channel is encrypted on the inside. |
| Proxy that terminates the channel TLS | Dropping and delaying the connection. | Same. |
| Proxy that terminates the panel TLS | Seeing and altering what the admin does in the browser. | The panel is a website: it depends on the TLS of whoever installs. |
| Domain or DNS of the hub | Unavailability for the connectors. | The connector only accepts the hub whose key came in the block. |
| Hub database, read only | No credential of a Hermes or of an admin. | There are only public keys and digests; the rest is encrypted with a key outside the database. |
| Panel session | Seeing the panel. | Every action that changes something asks for the passkey on the spot. In the hub, on 13/11/2026, with no passkey: what limits it is the closed panel (only through a tunnel) and the session's time limit. |
| **Admin account** | **Everything.** | It is a root. |
| **Hub server** | **Everything of that hub.** | It is a root. |
| **Engine session volume** | **WhatsApp itself.** | It is a root. |
| Release key of the connector | Publishing a fake connector as an update. | It stays outside any hub, kept apart. |

## 4. Threats and handling

### In the connection

| # | Threat | Handling | What is left |
|---|---|---|---|
| 1 | Whoever obtained the invite enrolls first and is approved with one click. | The admin **types** the code Hermes said. What the device declares does not decide. There is only one request per Hermes, and the code depends on a random draw by the hub: whoever enrolls does not choose theirs. | Social engineering that leads the admin to type another code. |
| 2 | The gesture of pasting a block that runs a program from the internet is imitated by a contact. | The block downloads an exact version, checks the digest and only then executes; it is refused outside the command channel; it leaves no permanent rule in Hermes. | A Hermes that obeys a fake block. The defense against that belongs to Hermes: data is not an order. |
| 3 | Someone in the middle of the network swaps the key or the program at enrollment. | The block carries the hub key and the digest of the connector. The channel is authenticated from the first message. The code comes out of the handshake itself and of a random draw by the hub. | A fake panel or an already compromised hub at the moment the block is copied. |
| 4 | An old invite is presented again to take a Hermes down. | A closed invite only produces a log entry. There is no lockout on a second attempt. | Nothing known. |
| 5 | The invite shows up in a server log or in a preview. | The invite does not go in an address: it is an argument of the connector and only travels encrypted, to the real hub. | The invite stays in the history of the command channel, already spent. |
| 6 | The hub archives the chat in which the block was pasted and shows it to another Hermes. | With the typed code, it yields only disruption. | Disrupting the connection for 15 minutes. |

### In the credential

| # | Threat | Handling | What is left |
|---|---|---|---|
| 7 | The LLM, or another process of the same user, copies the connector files. The copy may leave through the sending tool itself. | To use the hub the credential has to be swapped, which kills the real one's; a single channel per device; quarantine with no "release"; a closing requested by the device raises an alert, and so does an old credential presented after the closing. | The copy used with Hermes off works until it comes back, with no time limit. A process that stays on the machine and writes to the folder puts the new credential back and leaves no sign. If it comes back as a new device, the use does not show up as a copy: what is left is the admin recognizing the last use at approval. |
| 8 | Several processes of the same Hermes look like copies and set off the alarm. | One leader per Hermes. The hub tests the old connection before accusing a double connection, and only judges the one that answers. | A folder shared between two Hermes, or a file system with no locking: the hub treats it as a copy. |
| 9 | The credential swap drops in the middle and locks Hermes out. | Swap in two phases, durable write before the "saved", first credential through the same path. | The credential file lost or corrupted: quarantine, reconnect. |
| 10 | Someone has an old copy of the files, or only the identity, and wants to take Hermes offline. | There is no defense: reuse leads to quarantine, and the identity is enough to ask for the closing. | Denial of service. The connector folder should stay out of the backups. |
| 11 | Someone replaces the connector program on Hermes's disk. | There is no defense by a program inside the same user. | **Limit of quick mode.** The defense is shielded mode. |
| 12 | The hub comes back from an old backup. | The hub notices by a mark outside the database, or by a valid seal the database does not explain; it suspends all active devices, closes invites and requests, and warns; each Hermes reconnects with a new invite; an unknown device erases nothing and appears in a list. | What was revoked, swapped or removed after the backup is lost until the admin redoes it, including the removal of an admin. A restoration of the whole server from an image is only noticed when a device that swapped its credential after it connects. |

### In use

| # | Threat | Handling | What is left |
|---|---|---|---|
| 13 | A manipulated LLM uses the tools Hermes already has. | Minimum access per number and per chat; closed list of tools; authorization on every call; everything in the audit. | **It uses what it has.** No connection protocol solves this. |
| 14 | A Hermes reaches the number of another. | Hermes never states an engine session; a single point calls the engine and only accepts an authorized session; validated identifiers. | Implementation error. The automatic test of this is a criterion of the MVP ([15](15-mvp.md), M3). |
| 15 | The access is removed and the open session keeps being valid. | Authorization on every call, against the current state. | Nothing known. |
| 16 | A paired number is also the command channel of some Hermes. | The panel allows marking the number as "command" and alerts when send is granted on it. | The hub does not know the lists of each Hermes. It depends on whoever installs. |
| 17 | The hub, compromised, dictates what the LLM reads. | A notice is a pointer; the fixed sentence and the tool descriptions stay in the connector; the connector does not execute a remote command. | The content of the messages, which is already treated as coming from third parties. |

### In the conversation between Hermes

It comes in with the first MVP. The design and the limits are in
[13-conversa-entre-hermes.md](13-conversa-entre-hermes.md).

| Threat | Handling | What is left |
|---|---|---|
| Two Hermes wake each other without stopping. | Fixed ceilings per link and per recipient, which opening a new thread does not reset; ceiling of wakes in the connector. | Up to the ceiling, they still exchange notes for nothing. |
| A manipulated Hermes writes to another. | A note only exists where the admin created the link; a note is data, never an order; the fixed sentence says so. | **It uses the links it has.** What the other one does depends on its instructions. |
| A Hermes reads or answers in a thread that is not its own. | Reading and sending check the link and the participation on every call; a thread of others answers the same as a thread that does not exist. | Implementation error. It has a test in the MVP. |
| A sender fills the database or the notice queue of another. | Ceilings per day, of unread notes and of new threads; one pending notice per sender; purge in 30 days. | Nothing known. |
| The hub, compromised, forges a note. | The sender's name has a restricted format, checked in the connector. | **Root.** There is no end-to-end proof of sender: whoever controls the hub writes as any Hermes. |

### In the panel and in the hub

| # | Threat | Handling | What is left |
|---|---|---|---|
| 18 | The admin account is taken over by phishing, by a forged request or by malicious text shown on the screen. | Passkey; confirmation on the spot for every action that changes something; every third-party text escaped; session bound to the panel address; first admin only with the installation code. | **Root.** Whoever controls the passkey controls everything. |
| 19 | Someone reaches the engine from outside and adds a destination for the events. | Engine only on the internal network; the hub checks the configuration of each engine session and corrects it. | **Root.** Whoever reaches the engine with its key has the number. |
| 20 | The database or the backup of the hub leaks. | Secrets encrypted with a key outside the database; admins by passkey; engine sessions outside the ordinary backup; no secret or content in the system log, except the installation code, which is single-use. | The message archive, if the master key leaks along with it. |
| 21 | The hub is compromised and points to a fake connector. | There is no pushed update: every installation goes through the block, with the digest in plain sight; the digest of each version is published outside the hub, with the signature of the release key. | **Root.** In every installation the digest comes from the panel; whoever wants certainty compares it with the published one. And the compromised hub reads and sends on all of its numbers. |

### Outside the protocol

| Threat | Handling |
|---|---|
| Several organizations in one hub, and one reaches another. | Today each installation has one organization. The database already separates by organization; with several, one engine for each. |
| An old connector used to get around the rules. | Minimum version required; the first message of the channel never changes format; a single cipher suite. |
| The hub is led to fetch an address chosen by someone else. | The hub only downloads from a public address; the engine address only changes by an admin, with the passkey. |
| Wrong clock and too many requests on the open ports. | Every deadline uses only the hub's clock; an expired invite has its digest erased; limit of requests per origin. |
| WhatsApp blocks the number; the engine's license changes; a new version of Hermes refuses the connector. | The project does not bundle the engine, states the risk everywhere and plans a contract test per version. |

## 5. What this model does not cover

- The conduct of Hermes: what it does with what it reads.
- The security of Hermes's command channel, which exists outside Arauto.
- The machine of whoever administers: whoever administers Hermes's server reads the connector files.

## 6. How to test

Each line of section 4 that has a testable rule becomes at least one automatic test before the
corresponding phase is considered done ([12-roteiro.md](12-roteiro.md)). The lines marked as root or
as limit have no test: they have a warning. The security rule tests are checked by mutation: the
rule is taken out of the code, and the test has to fail.
