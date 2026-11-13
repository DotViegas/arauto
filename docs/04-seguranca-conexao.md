# 04 — Connection security

**Status: proposal v0.4**, to be validated with a prototype before implementing
([12-roteiro.md](12-roteiro.md), Phase 1). It specifies how a Hermes enters, comes back to and
leaves the hub. The terms are in the [glossary](glossario.md).

This design has already gone through several rounds of critical reading, and each one found flaws
the previous one did not see. v0.4 gathers the corrections from the third round onward: the hub
restored from a backup, the reconnect, the leave, the repeated commands, the verification code and
the order of writes on the hub's side. It only deserves trust after the prototype with randomized
rounds.

## Summary

```
INVITE             15 minutes, 1 use     only serves to ASK for a connection
CODE               6 digits              the admin TYPES what the Hermes answered
IDENTITY           born in Hermes        the private part never leaves the machine
SEALED CREDENTIAL  born in the hub       to use the hub it has to be swapped for another
```

| Question | Answer |
|---|---|
| What goes through the chat? | Only the invite, which dies on the first use, and the verification code, which is not a secret. |
| Who creates the secrets? | The identity is born in the connector. The sealed credential is born in the hub's vault. |
| Does any person see a long-lived secret? | No. None appears on screen, in a log or in a tool response. |
| What if the credential is copied? | Whoever uses the copy is found out when the real device presents its own again. The limits are in section 6. |
| What does the design not solve? | A taken-over admin account, a breached hub server and an exposed engine. See section 6 and [05](05-modelo-de-ameacas.md). |

## 1. Pieces used

No invented cryptography. Each step uses a known piece.

| Use | Piece |
|---|---|
| Drawing of every secret | The operating system's generator. Nothing comes from date, time, name or counter. |
| Connector ↔ hub channel | Noise `XK_25519_ChaChaPoly_BLAKE2s`, inside the websocket. One fixed suite, with no fallback mode. Mutual authentication and forward secrecy. |
| Device identity and hub key | X25519. |
| Invite | 32 randomly drawn bytes. The hub keeps only the SHA-256 digest. |
| Verification code | HKDF-SHA-256 over the digest of the handshake and a number drawn by the hub, reduced to six digits without bias. |
| Seal of the credential | AEAD XChaCha20-Poly1305, with the organization key. |
| Signature of the connector's versions | Ed25519. |
| Panel login | Passkey (WebAuthn). |
| Swap of the credential | Rotation with reuse detection, the standard of credential renewal in OAuth (RFC 9700). |

The channel is encrypted inside the websocket so as not to depend on whoever terminates the TLS. In
real installations the TLS terminates at a proxy that is not the project's, and in an internal
network it may not even exist. With Noise, the security of the channel is the same in both cases.

## 2. Keys of the hub

| Key | Where it stays | What it is for |
|---|---|---|
| **Master** | A file outside the database and outside the backup of the database. | Encrypts the other keys and the secrets kept in the database. |
| **Organization** | In the database, encrypted by the master key. | Seals the credentials and encrypts the message archive. |
| **Hub** (X25519) | In the database, encrypted by the master key. | It is by it that the connector recognizes the hub. The public part goes in every block. |

Changing or losing the hub key forces every Hermes to be connected again. It is an accepted cost:
connecting is exactly what the project makes cheap. Before that, each Hermes needs a local leave,
done by a person (C10): the connector does not change hubs by itself.

The release key belongs to the project, not to the hub. It signs each published version of the
connector. In the first version of the project there is no automatic update: every installation
goes through the block, and the check is the digest that comes in it
([11-conector.md](11-conector.md), section 7).

## 3. Flow

### C1. Creating the invite

The admin clicks "Connect Hermes" and gives the name. The hub draws 32 bytes: that is the invite
(`arc1` + base32). It keeps **only the SHA-256 digest** of it, valid for 15 minutes. The invite
appears a single time, inside the block.

Whoever reads the database cannot use the invite. When it expires, the digest is erased: a clock
that is behind does not make it valid again.

### C2. The block

The block carries the name of the Hermes, the download address of the connector in an exact version,
the SHA-256 digest of that version, the address of the hub, the public key of the hub and the
invite. It does not use `curl | sh`. The proposed text is in
[02-como-funciona.md](02-como-funciona.md).

The invite goes as an argument of the connector. It does not appear in an address, so it does not
stay in a server log and is not read by a link preview. The download address is public and carries
no secret.

### C3. Enrollment

The connector:

1. Draws the device identity and saves it, in a folder only for that request, before talking to the
   hub. **New invite, new identity:** an identity is only reused when the same invite is presented
   again. The connector recognizes the same invite by its digest, kept in the folder of the
   request.
2. Opens the Noise XK handshake. The static key of the connector is the **device identity**; that
   of the hub is the one that came in the block. A fake hub does not complete the handshake.
3. Sends, already encrypted to the hub: the invite and the data of the device (machine name,
   connector version, Hermes version, system).

The hub looks up the digest of the invite and decides:

| Situation | Response |
|---|---|
| Invite issued and within the deadline | Creates the device as **pending**, marks the invite as used and responds with the confirmation deadline. |
| Invite used, same identity, request still pending | It is the same request. It changes neither the deadline nor the attempts. The code of this enrollment becomes the valid one (C4). On the sixth enrollment the request is denied. |
| Invite used, same identity, request already resolved | Tells the exact situation to whoever proved to be the same: already approved (AR-104), denied (AR-301) or expired (AR-302, AR-303). It changes nothing. |
| Invite used, another identity, request still pending | Refuses (AR-102) and **alerts the panel**: the block ended up somewhere else. The first request is not touched. |
| Any other case | A single refusal (AR-101), without saying why. It only produces a record in the audit. |

The invite only travels encrypted, inside a channel that only the real hub can open. In the same
handshake the connector proves that it has the identity it is enrolling.

A closed invite **never brings down a connected Hermes**.

**One request per Hermes.** There is only one request in progress per Hermes. Generating a new
invite for a Hermes cancels the issued invite and closes the request that is pending, or approved
and still without a confirmed credential. A request that went to quarantine before confirming the
first credential also counts as a request in progress: the new invite closes it in the same way. The
device in use is not touched.

### C4. Verification code

The two sides compute, each on its own, six digits from two things: the digest of the handshake and
a number that the hub draws and only sends, through the already encrypted channel, after the last
message of the handshake. Without that number, whoever enrolls would be able to choose their own
code, redoing the last message until it gives what they want. The hub **never sends or shows** the
code.

- The connector shows the code and ends.
- The Hermes says the code in the command channel.
- The admin types the code in the panel.

The panel only approves if what was typed matches the pending request **of that Hermes**, which is
only one (C3). Five errors close the request. The deadline is 10 minutes from the first enrollment.

If the command of the block runs again on the same Hermes, there is a new handshake and, with it, a
new code. **The most recent one is valid.** The connector says, in its response, that the previous
code is no longer valid. The hub accepts up to five enrollments per request: with the hub's draw in
each one, repeating does not help to reach a chosen code.

Why type instead of compare and click: whoever obtains the invite before its use can create a
request with the right name. If a click were enough, that request would be approved. By typing, only
the request whose code the real Hermes said is approved.

What the device declares (machine name, version) appears on the screen as "informed by the device,
not verified". It decides nothing.

### C5. Approval

With the right code, the device becomes **approved**. On a first connection, the Hermes has **no
access to any number**: the accesses are given afterwards, in the matrix ([08](08-permissoes.md)).
On a reconnect, the accesses the Hermes already had start to apply to the new device as soon as it
confirms the first credential (C10).

An approved device is at **generation zero**: it does not have a **confirmed** credential yet. It
stays at generation zero even after the hub issues the first one, until its "saved". It receives
the first one through the same swap as all the others (C7).

### C6. Registration in Hermes and waiting

At the end of the enrollment the connector registers itself in Hermes as a tool server, through the
official command (`hermes mcp add`). The entry contains no secret: only the path of the program and
the argument `mcp`. It is the only change that stays in Hermes's configuration.

Hermes starts the connector in less than a minute. While the device is pending, the connector opens
the channel **waiting**: it proves the identity, has no WhatsApp tool at all and waits.

| What happens to the request | What the connector does |
|---|---|
| Approved | Does the first swap right there (C7). |
| Denied or expired | Shows the error code. If the folder **of that request** does not have a credential yet, it discards the folder and, if there is no other device, removes the registration. |
| A second waiting connection appears with the same identity, and the first one responds | The hub denies the request and alerts: the identity was copied before the approval. |

**The connector never discards, on information from the hub, a folder that already has a
credential.** It only discards a request that has not received any yet. The folders left over when
another device takes over are a separate case: they go by decision of the connector, after the new
one takes over and after the hub responds (C10).

If the enrollment worked and the registration in Hermes failed (AR-601), the request stays standing
in the hub. Repeating the command tries the registration again.

### C7. The swap of the credential

Every channel connection of an approved or active device begins with a swap. The channel session
only exists after it. The short connections of `join` and of `leave` are not channel connections
(C9).

```
connector -> hub    handshake (proves the identity) + the credential it has
hub                 checks that whoever presented is still on the line
hub                 saves the new one as pending
hub -> connector    NEW credential
connector           saves the new one, durably
connector -> hub    "saved"
hub                 saves: the new one comes into force; the previous one dies
                    only now does the session serve tools and notices
```

**Rules**

1. Every issued credential has a new generation and a randomly drawn identifier inside the seal.
   The seal binds the organization, the device identity and the generation.
2. The hub keeps, per device, the generation and the digest of the **current** one and of the
   **pending** one, and the highest generation ever issued, even after the device is closed. It
   compares the digest, not only the number: after a restoration of the hub, a generation number
   may repeat.
3. The hub only judges the credential of whoever proved the identity in the handshake **and**
   answered a check on the same connection. An abandoned attempt, which arrives late through the
   network, does not answer and is discarded without consequence. The connector closes one attempt
   before opening another.
4. **The session only serves tools and notices after the "saved".** Whoever wants to use the hub
   has to confirm the swap, and confirming kills the previous credential. That is what makes the
   use of a copy show up when the real device comes back. The limits are in section 6.
5. The connector only says "saved" after the write is guaranteed on disk, in the file and in the
   folder.
6. The hub only sends the new credential after saving, durably, the pending one and the highest
   generation. It only opens the session after saving the promotion.
7. A "saved" that does not refer to the pending one is refused, and the connection is closed.
8. **After generation zero, the hub never issues a credential on identity alone.**

**What the hub does with what the connector presents**

It applies to an approved or active device. In any other state the hub responds with the situation
of the device and swaps nothing. Two checks apply in all states:

- A valid seal of a generation higher than the database knows is a matter for C11: it may be proof
  of restoration.
- A device that was revoked, replaced or expired by inactivity and that presents a valid seal
  **older** than the last current one and the last pending one produces the alert "reuse after the
  closing": someone used a copy, and the device was closed before the real one came back.

| The connector presents | The hub |
|---|---|
| The current one (or nothing, if the device is at generation zero) | Accepts and issues a new one, which becomes the pending one. If there already was a pending one, it is discarded. |
| The pending one | Accepts, promotes the pending one to current, and the previous current one dies. Issues another. |
| Any other one already issued, or nothing when the device is already past generation zero | Quarantine and alert, with the reason "reuse" (C9). |
| A seal that does not open, or that belongs to another device | Quarantine and alert, with the reason "unreadable credential". |
| A valid seal, of a generation higher than all the ones the hub knows | It is proof that the hub came back from an old backup. Turns on restored mode (C11). |

The swap has two phases so that a drop in the middle does not lock the Hermes outside:

| Where it dropped | What the connector has | On the way back |
|---|---|---|
| Before saving the new one | The current one | Presents the current one and receives another. |
| On the first swap, before saving the first one | Nothing | Presents nothing. The device stays at generation zero and receives another. |
| After saving, before the hub hears the "saved" | The pending one | Presents the pending one, and the hub promotes it. |

A connection that ends without "saved" is recorded. Three in 24 hours from the same device produce
an alert, even with confirmed connections in between: it may be a bad network, it may be someone
testing a copy.

**The 24-hour swap.** Every 24 hours the hub repeats the swap inside the open session, without
bringing down the channel. The current one counts as presented. The session keeps serving while the
connector saves. The hub does not issue another while there is a pending one without confirmation:
if the "saved" does not arrive in 60 seconds, it closes the connection, and on the way back the
usual rows apply.

A Hermes that loses the credential file and keeps the identity falls into the third row of the
table: quarantine, and the way out is to reconnect.

### C8. Use

Through the channel pass tool calls (Hermes → hub) and notices (hub → Hermes).

- **Authorization on every call.** The hub checks the state of the device and the access (Hermes,
  number, verb, chat) on every call and on every notice. Taking an access away applies on the next
  call.
- **The hub does not send free text.** The notice is a pointer: which number, which chat, how many
  messages. The sentence that wakes the Hermes is fixed and stays in the connector.
- **The descriptions of the tools stay in the connector.** The hub only says which names are
  allowed.
- **The connector does not execute a command from the hub.** There is no message from the hub that
  runs something on Hermes's machine, nor one that erases a device that has already worked. The
  first version has no update pushed by the hub.
- **The version is checked right after the handshake, before the swap.** A connector below the
  minimum version receives AR-401 and nothing else. The exception is the closing request (`leave`),
  accepted in any version. A message from whoever has not proved the identity yet changes no state
  at all in the hub.

### C9. When the hub stops trusting the credential

There is only one channel per device ([11-conector.md](11-conector.md) explains how this is
guaranteed on Hermes's side). The channel is the connection the leader keeps, waiting or in
session. The short connections of `join` (enrollment) and of `leave` (closing request) prove the
identity, carry a single request and close: they do not present a credential, do not become a
session and do not count as a double connection.

With that, two signs mean a copy:

| Sign | Response |
|---|---|
| **Reuse:** a credential arrives that was already swapped, or none when there should be one | Quarantine and alert. |
| **Double connection:** a presentation arrives that would be accepted (a credential, or nothing at generation zero) while another connection of the same device responds | Refuses the second one, quarantine and alert. |

Quarantine also comes without a copy, when the credential file is lost or damaged.

Before treating it as a double connection, the hub tests the old connection. If it does not respond
in 3 seconds, it is taken for dead: the old one is closed and the new one goes on. **Every time this
happens to a session that was serving, the panel alerts**: "the connection of {name} was taken by
another, coming from {address}". The test and the heartbeat travel inside the encrypted channel: a
proxy on the path does not answer for them.

The real connector notices that it was taken for dead in two ways: by the close the hub sends, or,
if its network is being held, by the lack of heartbeat, in up to 90 seconds. In both cases it
**reconnects at once**. If someone took its place, it shows up on that return: as reuse, if the
other one has already swapped the credential, or as a double connection, if it has not swapped yet.

Therefore: with the real Hermes on, in a version the hub accepts and reaching the hub, the copy is
stopped in seconds; in the worst case, when its connection was silent without it knowing, in up to
90 seconds. It is not so when whoever has the copy also holds the network of the real one: the copy
is found out when the real one talks to the hub again. And whoever holds the network can wait for
the connection of the real one to drop for lack of heartbeat before coming in: then there is no
taken connection, and the panel shows only a return coming from another address. It is also not so
when the real connector fell below the minimum version: it stops using the channel and no longer
presents its credential (section 6, item 3).

A new network or country produces an alert, never a block: residential and carrier addresses
change. This alert and the one for swaps without confirmation (C7) can be provoked by whoever
controls the network: alone, they are not a reason to revoke.

### C10. Quarantine, revoke, reconnect and leave

**Quarantine.** The hub stopped trusting the credential of that device. Channel closed, calls
denied, notices held. There is no "release": after a sign of a copy there is no way of knowing
which side is the real one. The way out is to reconnect or to revoke.

**Revoke.** Revoking a Hermes closes everything that is its own: the device, the request in progress
(pending or approved) and the issued invite. The device becomes revoked and the line is kept as a
record. The channel drops in seconds and the next call is already denied. The connector stops using
the channel and shows AR-502. It erases nothing; the cleanup is done by `leave` or by the next
`join`.

**Reconnect.** A new invite for the same Hermes, with an old device still in the folder:

1. Each device has its own folder, with identity, credential and data of the hub. The `join`
   creates the folder of the new request, without touching that of the current device.
2. While the new request is not approved, the current device goes on as it is. If the request is
   denied or expires, only the folder of the request is discarded.
3. When approving, the panel shows the last use of the current device, with date and address, and
   asks whether the admin recognizes it. If the Hermes was off and there was use, someone used a
   copy. If the admin does not recognize it, the copy alert goes out and the approval goes on. The
   old device, if it was active, goes to quarantine right there; if it was already suspended or in
   quarantine, it stays as it is. When the previous device expired by inactivity, the panel shows
   its last use in the same way, and it stays expired.
4. Once the first credential of the new device is confirmed, the hub makes it active and makes the
   old one **replaced** in the same write, whether it comes from active, from suspended or from
   quarantine. The connector starts using the new folder in a single rename.
5. **One machine, one device.** If the invite was from another line of the panel, the hub has no
   old device to replace. So, on taking over the new device, the connector asks for the closing of
   the old one, unless the hub has already given it as replaced, and of each other identity that
   is left over in the folder, such as a closed request that already had a credential. "Already
   closed" and "I do not know this device" count as confirmation. Each folder is only erased after
   the hub's response; until then, the leader repeats the request on every start and every time
   the channel comes back. The alert of that closing shows the last use of the old device, as in
   step 3. When nobody asks for the closing, the old device stays active in the hub with no
   machine, until the admin revokes it: that is the case of the local leave and that of the old
   folder lost (rebuilt machine) with an invite from another line of the panel.

The accesses belong to the Hermes, not to the device, and start to apply to the new device.

**Leave.** The `leave` command asks the hub for the closing of the device itself, waits for the
response and only then erases the files and removes the registration.

- The request applies to **each identity** there is in the folder: the current device and, if there
  is one, a request in progress. The connector only erases after all the responses.
- If the leader has the channel open, the request goes through it. If it does not (device in
  quarantine, suspended, revoked, or connector stopped), `leave` opens a short connection, proves
  the identity and asks.
- "Already closed" and "I do not know this device" count as confirmation.
- If the hub does not respond, the connector **does not erase the identity**: it warns with AR-505
  that the leave is pending and that the device needs to be revoked in the panel.
- **Every closing asked for by the device itself produces an alert in the panel**, with the address
  it came from and the last use of the device. Without that, whoever used a copy could close the
  device to erase the trail.

**Local leave.** `leave --local` erases the files and the registration without talking to the hub.
It exists for when the hub no longer exists or changed its key. It is for a person to run: the
device stays valid in the hub until it is revoked in the panel.

Whoever has the identity of a device can close it. It is denial of service, of the same kind as
presenting an old copy, and the alert shows where it came from.

### C11. Hub restored from a backup

A hub that comes back from an old backup forgets what happened after it: approvals, credential
swaps, quarantines, reconnections, revocations and access changes. To it, a credential that had
already died is valid again and a revoked device is active again.

**How it notices.** In two ways:

- A **mark outside the database**, beside the master key: the number of the last security event of
  the audit. It counts every event that changes who may do what: invite used or canceled, approval
  and denial, confirmed credential swap, quarantine, reconnection, revocation, access change and
  admin change. The mark only advances after the event is guaranteed on disk in the database: a
  drop between the two leaves the mark one event behind. This does not accuse a restoration for
  nothing; the cost is not noticing a restoration that loses only that last event. At start, a
  database behind the mark, or a database with data and a missing mark, means that it came back
  from a backup.
- A **valid seal the database does not explain**: of a generation higher than all the ones it knows
  for that device, or from a device it does not know. Only the hub creates credentials, and it
  saves the generation before sending (C7, rule 6). A valid seal the database does not know is a
  database that is behind. It does not apply to a device already marked by a previous restoration
  (below).

If the whole server is restored from an image, the mark comes back with it and only the second way
works: the hub only notices when a device connects that swapped its credential, or that was
connected, after the image.

**What it does: it turns on restored mode.**

- **All active devices become suspended**, and the open sessions are closed. The hub no longer
  knows which credentials are valid, which devices were replaced and which were revoked. A
  suspended device receives AR-506, stops using the channel and erases nothing.
- **Issued invites and requests in progress are closed**: the pending ones and the approved ones
  still without a confirmed credential. A request that was already in quarantine stays as it is. An
  invite already used after the backup would be valid again for the rest of the deadline.
- **Every device that already existed in the database is marked**, in any state. A marked device no
  longer serves as proof of restoration: a seal of a higher generation coming from it does not turn
  the mode on again, it only produces a record. The rest still applies to it: AR-504 with the
  listing (below) and the alert of reuse after the closing (C7).
- **The numbering of the audit continues from the mark.** The database is ahead of it again, and
  restarting the hub does not accuse the same restoration again. Turning on the mode is a single
  write in the database (suspensions, closings, markings and the first new event), and only then
  does the mark advance. If the mark was missing, the numbering goes on from the last number of the
  database and the hub recreates the mark.
- **There is no "reactivate".** Each Hermes leaves suspended by **reconnecting**, with a new invite,
  or is **revoked**. Reconnecting creates a new identity and credential, and with that any copy of
  the old files of the connector stops being of use, whether taken before or after the hub's
  backup.
- The panel shows, on any screen: "The hub came back from a backup of {date}. What was approved,
  revoked, reconnected or changed in the accesses after that was lost. Reconnect each Hermes and
  check the accesses and the list of admins."
- A device that the database does not know, or that in the database was a request closed by the
  restoration, and that presents a valid seal, is a Hermes connected or reconnected after the
  backup. It receives AR-504 and **erases nothing**; the panel lists it, with the time, the address
  and what it declares, and keeps its identity. Its path is also to reconnect.
- The mode ends when no suspended device is left.

**New restoration with the mode on.** A database behind the mark, or a database with data and a
missing mark, at a start, or a seal of a higher generation coming from a device created after the
beginning of the mode, is another restoration: the mode starts over and applies to all devices,
including the ones created during the previous one. A seal of a higher generation coming from a
marked device, or a seal from an unknown one already listed, does not start the mode over: it only
produces a record.

Even with the continuous backup of the control database ([03](03-arquitetura.md), section 8), in
which the loss is of minutes, everyone reconnects: the hub has no way of knowing what it lost. It is
a cost similar to that of changing the hub key, accepted for the same reason.

**Open.** After the mode has ended, a device the database does not know may still appear for the
first time, with a valid seal from the lost timeline. It would look like proof of a new restoration
and would suspend everyone once more. It remains to decide how to tell them apart: one idea is to
write the issue date in the seal and treat as a leftover what was issued before the end of the last
handled restoration. It is a deliverable of Phase 1.

## 4. What each side keeps

**In the hub**

| Data | How |
|---|---|
| Invite | Only the SHA-256 digest, the deadline and the state. The digest is erased when the invite expires. |
| Device identity | Only the public part. |
| Sealed credential | Only the generation and the digest of the current one and of the pending one, and the highest generation issued. |
| Login of the admins | Passkeys, which are public keys. There is no password. |
| Panel session | Only the digest of the identifier. |
| Hub key, organization key, engine credential | Encrypted with the master key, which stays outside the database. |
| Message archive | Encrypted with the organization key. |
| Outside the database | The master key and the audit mark. |

**In Hermes**, in the folder `~/.arauto`, which only the owner reads: for each device, the identity
and the most recent credential the connector received, and data with no secret. The complete list is
in [11-conector.md](11-conector.md), section 2.

## 5. What the design sets out to guarantee

It holds while the hub, the admin's account and the system's generator are intact. These are goals
of the design: they only become a guarantee after the tests of Phase 1 and Phase 4.

1. A chat or block leaked after the connection contains nothing that connects.
2. Leaked before its use, it gives no access without an authenticated admin typing the attacker's
   code.
3. The private part of the identity is born in Hermes and is never transmitted or shown.
4. No long-lived secret goes through the chat, appears on screen or goes into a log.
5. Whoever takes a copy of the connector's files outside and uses it is found out when the real
   device presents its credential again. If the real Hermes is on, with the connector in a version
   the hub accepts, and reaches the hub, this happens in seconds; in the worst case, in up to 90
   seconds. The hub alerts and the device stays stopped: in quarantine or, if whoever copied had
   already closed it, revoked.
6. A copy stops giving access after the first confirmed swap that comes after it.
7. Revoking and changing access apply on the next call.
8. A Hermes only reaches the numbers that were given to it. Through the channel, a device does
   nothing of an admin: at most it asks for its own closing.
9. On the channel between connector and hub, whoever terminates the TLS on the path only sees
   encrypted data.
10. Whoever only reads the hub's database cannot pass for a Hermes.

## 6. What is not guaranteed

1. **"The LLM does not reach the credential."** In quick mode, the LLM's terminal runs as the same
   user as Hermes: it reads and writes the connector's files. What holds: it does not open the
   seal, it does not need the secret, and whoever takes a copy outside is found out when the real
   device comes back. But a process that stays on the machine and writes in the folder can put the
   new credential back before the connector returns, and then there is no sign. The strong
   guarantee belongs to shielded mode.
2. **"Nobody has access."** Whoever administers Hermes's machine reads the files. The true meaning
   is: nobody needs to see, type or keep the secret.
3. **A copy used with the real Hermes off works until it comes back.** There is no time limit. If
   it never comes back, the copy is never found out. The inactivity period, of 30 days
   ([decisoes/0021](decisoes/0021-prazo-de-inatividade.md)), only catches the copy used for the
   first time after the period. A Hermes whose
   connector fell below the hub's minimum version counts as off: it stops using the channel, and a
   copy used with a current connector works until the admin reconnects that Hermes. On the
   reconnect, the use of the copy may not show up as a copy (item 4). It is decided that the hub
   will check the credential of that connector before refusing the version
   ([decisoes/0030](decisoes/0030-conector-desatualizado.md)); while Phase 1 does not fix how, this
   limit holds.
4. **If the real Hermes comes back as a new device, the use of the copy does not show up as a
   copy.** It happens when it reconnects on a rebuilt machine, without the old files. The copy
   loses access, and what is left is for the admin to recognize, at approval, the last use of the
   old device.
5. **Whoever has the copy and also holds the network of the real Hermes uses the hub for as long as
   they hold it.** If they come in with the connection of the real one still open, the panel
   alerts that it was taken. If they wait for that connection to drop before coming in, the panel
   only shows a return coming from another address. Quarantine only comes when the real one talks
   to the hub.
6. **Whoever has an old copy, or only the identity, can take the Hermes off the air.** They gain no
   access, but provoke the quarantine or the closing. That is why the connector's folder should
   stay out of the backups of Hermes's volume.
7. **After the hub comes back from an old backup**, everything that was revoked, swapped or taken
   away after it is lost. The hub suspends all active devices and each Hermes reconnects (C11). But
   a restoration of the whole server from an image is only noticed when a device connects that
   swapped its credential, or that was connected, after it: until then, a credential that had
   already died is valid again and a revoked device works again. An admin removed after the backup
   has access again until removed again. And, after the restoration has been handled, a device from
   the lost timeline that the hub never saw can still suspend everyone once (C11, Open).
8. A manipulated LLM uses the tools Hermes already has, without stealing anything. A connection
   protocol does not solve that; only minimal access, limits and audit do.
9. An LLM already manipulated at the time of the connection can lie about the code or run something
   else. Against that, only a person running the command, or shielded mode.
10. The copy of the credential can go out through the sending tool itself, with no network command
    that Hermes's guard sees.
11. A taken-over admin account or a breached hub server bring everything down.
12. In every installation through the block, the digest of the connector comes from the panel. A
    breached hub points to a fake program. That is why the digest is also published in the
    project's releases.
13. Whoever reads the chat in real time can disturb the connection. They cannot get in. Whoever
    controls the network path between Hermes and the hub prevents the connection for as long as
    they control it: the design protects who gets in and what passes, not availability.
14. Social engineering that leads the admin to type the code of another device.
15. Revoking a Hermes does not turn off a copy of the engine sessions. Whoever copied them is a
    linked device of the number, and only leaves through the phone.
16. The panel depends on the TLS of whoever installs. The channel encrypted on the inside protects
    the connector, not the admin's browser.
17. If the connector's folder is shared between two Hermes, or the file system has no lock, the hub
    treats the two as a copy and puts the device in quarantine.
18. A lost or damaged credential file leads to quarantine, even with no copy at all.

## 7. To be proven before implementing

The single list of what needs to be proven is in [12-roteiro.md](12-roteiro.md), Phase 1. On it
depend, in this document: the registration of the connector in Hermes (C6), the swap of the
credential with drops, delays and a connector that never confirms (C7), the reconnect with a drop
in the middle (C10), the restoration of the hub (C11) and the size of the channel's messages, which
in Noise is 64 KB each.

Not yet written, and deliverables of Phase 1: the format of the channel's messages, the check of
rule 3, the exact content of the seal, the exact function of the verification code and the handling
of the credentials left over from an already handled restoration (C11). Also left for Phase 1 is the
way the hub checks the credential of a connector below the minimum version before refusing it: the
rule is decided ([decisoes/0030](decisoes/0030-conector-desatualizado.md)), the order of the
messages is missing (section 6, item 3).

## 8. Alternatives considered

**TLS 1.3 with a signed challenge, without Noise.** It is simpler to implement and is enough when
the proxy belongs to whoever installs. It is weaker in two common cases: an internal network without
TLS and a third-party proxy that terminates the TLS. It stayed as plan B, recorded in
[decisoes/0012-canal-cifrado-por-dentro.md](decisoes/0012-canal-cifrado-por-dentro.md).

**The device flow Hermes already has** (OAuth, RFC 8628). It installs nothing and puts no secret in
the chat. But the credential would be kept by Hermes without a swap with reuse detection, there is
no way to wake Hermes through it, and the hub would have to be an authorization server. It may
serve, later on, agents that only query.
