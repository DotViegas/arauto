# 11 — The connector

**Status: proposal.** The program that runs beside Hermes.

## Summary

```
arauto-connector join     asks the hub for a connection and registers itself in Hermes
arauto-connector mcp      the tool server that Hermes starts
arauto-connector status   shows the situation, with no secret
arauto-connector register saves the entry in Hermes again, with the reference to the key of
                          its local API (for a Hermes connected before 15/11/2026)
arauto-connector leave    asks the hub for its own closing and cleans the folder
                          (leave --local: cleans without the hub; it is for a person)
arauto-connector await-reply THREAD [--minutes N]
                          waits, through the leader, for an answer in a thread, and prints one
                          fixed line (proposal, 15/11/2026, decision 0048)
```

One file only, with nothing else to install. It opens no network port, runs no command from the hub
and shows no secret. The only thing it discards on information from the hub is a connection request
that has not received a credential yet.

## 1. Commands

| Command | What it does |
|---|---|
| `join HUB KEY INVITE` | Creates the folder of the request with a new identity and does the enrollment with the hub of that key. If the hub accepts, it copies itself to the fixed path `~/.arauto/arauto-connector` (through a temporary file and a rename, because the old program may be running), registers the tool server in Hermes and shows the verification code. Then, only when this Hermes has no key for its local API, it makes one (section 6). It ends in seconds, or up to a minute later when it looks for a key. With `--no-register`, for development, it skips the registration. |
| `mcp` | What Hermes starts. It speaks MCP over stdio with Hermes and keeps the channel with the hub. |
| `status` | Hub, version, situation of the device, whether the admin allowed waking by WhatsApp. It reads the local files and asks the leader through the local socket. It does not open a connection of its own with the hub. |
| `register` | Proposal, 15/11/2026 ([decision 0046](decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md)). For a Hermes that is already connected: runs the same registration as `join` (section 4) and records in the folder of the device whether the entry has the reference to the key. It does not contact the hub, does not ask the leader anything, does not touch the identity, the credential or `current`, does not restart Hermes and makes no key for its local API. With no device in the folder it runs no Hermes command and says so. A running Hermes keeps the entry it loaded: the new one takes effect after Hermes restarts its gateway. After `register`, the operator restarts Hermes's gateway; until then the running connector keeps the old entry, and `status` says whether the key was received. |
| `leave` | Asks the hub for the closing of its own device. With the answer, it deletes the files and removes the registration in Hermes. It then prints a fixed line saying that the Arauto notes the Hermes may have saved in its memory stay there until the Hermes removes them (proposal, 15/11/2026, [decision 0047](decisoes/0047-notas-do-arauto-na-memoria.md)). |
| `await-reply THREAD [--minutes N]` | Proposal, 15/11/2026 ([decision 0048](decisoes/0048-resposta-na-conversa-que-perguntou.md)); built, not tried on a real Hermes. Waits for an answer in a thread and prints one fixed line. THREAD must be a UUID; otherwise it prints the usage line, exits with 2 and never shows the argument. N is from 1 to 120 minutes, default 20. It talks only to the local socket of the leader, like `status`: no connection of its own to the hub, no port, no new variable. Exit codes: 0 an answer arrived; 11 no answer in time; 12 cannot wait; 9 the leader is still gone at the deadline; 2 usage. A refusal that may pass (AR-203, AR-207, AR-710: for example a fresh leader without its channel session yet) is tried again every 5 seconds until the deadline, and then exits 12 with that code. The output is fixed text that carries only a validated thread, a validated name of a Hermes, the minutes, or a code of the catalog with its catalog sentence. It never reads the thread: no note text appears in it. |
| `leave --local` | Deletes the files and the registration without talking to the hub. It exists for when the hub no longer exists or changed its key. It is for a person to run: the device stays valid in the hub until it is revoked in the panel. |

### Cases of `join`

| Situation | What happens |
|---|---|
| Same invite again, request still pending | It is the same request, with the same identity: the connector recognizes the invite by the digest kept in the folder of the request. The verification code is another one, and the most recent is the valid one. The hub accepts up to five enrollments per request. |
| Same invite again, request already approved | The hub answers AR-104. Nothing changes in the hub: the connector redoes the registration in Hermes if it is missing or if the record says the entry has no reference to the key of the local API (see the next row), and makes a key for that API only when Hermes has none (section 6). |
| Re-registration (proposal, 15/11/2026) | The record in the folder says whether the entry was saved with the reference to the key (section 2). A rejoin or an AR-104 whose record says "with the reference" does not save the entry again; it still asks Hermes whether its key is set (section 6). One that says "without" or says nothing (a connector from before 15/11/2026) saves the entry again over the old one. |
| Same invite again, request denied or expired | The hub answers AR-301, AR-302 or AR-303. The connector discards the folder of that request only if it has no `credential.seal` yet; if it has one, it discards nothing and shows the error. |
| The hub refuses the invite (AR-101, AR-102) | It discards the folder it has just created for this request. Nothing else changes on the machine. |
| New invite, with an old request with no credential sitting in the folder | It discards the old request and creates another, with a new identity. |
| The enrollment worked and the registration in Hermes failed | It shows AR-601. The request still stands in the hub; running it again tries the registration. |
| There is already a device in the folder, of the same hub | It is treated as a reconnect, whatever the line of the invite in the panel. The current device keeps working until the new one has its first credential confirmed. When the new one takes over, the connector asks for the closing of the old one, if the hub has not yet given it as replaced, and of every other identity that is left in the folder; "already closed" and "I do not know this device" count as confirmation. Each folder is only deleted after the answer of the hub; until then, the leader repeats the request at every startup and every time the channel comes back. |
| There is already a device in the folder, of another hub | It refuses (AR-105). Only after `leave`; if the old hub no longer exists or changed its key, after `leave --local`. The same address with another key counts as another hub. |

At the end, `join` tells the running leader, through the local socket, that there is a new request.
If the program was swapped for a new version, it asks the leader to exit, so that Hermes starts it
again already in the new version. This depends on a proof: Hermes restarts a tool server that
exits ([12-roteiro.md](12-roteiro.md), Phase 1). **Result of 15/11/2026, one Hermes (v0.22.5,
Docker, Linux): a running gateway did not start a killed connector again in 15 minutes**
([hermes-fatos.md](../referencia/hermes-fatos.md), K5), so with a single copy the connector stays
gone until the gateway restarts. This is left for a decision. The alternative is for the leader to
swap itself for the new program, without closing the connection with Hermes.

### Cases of `leave`

| Situation | What happens |
|---|---|
| The leader has the channel open | The request goes through the leader. With the confirmation, it deletes everything and removes the registration. |
| There is more than one identity in the folder (a reconnect in progress) | It asks for the closing of each one. It only deletes after all the answers. |
| There is only a pending request | It asks the hub to close the request, which becomes denied. With the answer, it deletes. |
| There is no channel: device in quarantine, suspended, revoked, or connector stopped | `leave` opens a short connection, proves the identity and asks. |
| The hub answers "already closed", or that it does not know the device (AR-504) | It counts as confirmation: there is nothing to close. It deletes everything and removes the registration. |
| The hub does not answer, or is no longer the one of the stored key | **It does not delete the identity.** It shows AR-505: the leave is pending and the device needs to be revoked in the panel. Running it again tries once more. If the hub no longer exists or changed its key, what is left is the local leave, done by a person. |

## 2. Files

Everything in `~/.arauto`, a folder only the owner reads. The variable `ARAUTO_HOME` points to
another folder. The registration in Hermes always carries the folder, in `--home`, also when it
is the default one: in a container, Hermes v0.22.5 gives the terminal of the agent one `HOME` and
the server of tools it starts another (checked on 14/11/2026, `referencia/hermes-fatos.md`)
([15](15-mvp.md), section 4).

```
~/.arauto/
  arauto-connector            the program, at the fixed path registered in Hermes
  arauto-connector-{VERSION}   the file downloaded by the block; it can be deleted after the join
  current                     says which device folder is in use
  devices/
    {id}/                     one folder per device or request
      identity.key            SECRET: the identity of the device
      credential.seal         SECRET: the most recent credential received from the hub
      hub.json                address and public key of the hub, digest of the invite of the
                              request, local situation, and (proposal, 15/11/2026) whether the
                              entry in Hermes was saved with the reference to the key of its
                              local API (`key_reference`: true, false, or absent when an older
                              connector saved it)
  state.json                  last notices handled and the tool list of the last session
  lock                        lock file of the leader
  channel.sock                local socket between the copies of the connector
```

- **Each device has its own complete folder.** A new request is born in its own folder and receives
  the first credential there. The change of device is a single rename of the file `current`. At
  startup, the connector follows what `current` points to and tracks, apart, a request not yet
  concluded. If the folder of the request already has a credential, it connects with it: accepted by
  the hub, it concludes the swap of `current`; refused, it goes on with the current device.
- **Durable write.** It writes to a temporary file, forces the write of the file, renames it and
  forces the write of the folder. Only then does the connector say "saved" to the hub.
- **What the connector discards on information from the hub:** only the folder of a request that
  has no `credential.seal` yet, when the hub refuses the invite or says that the request was denied
  or expired. A folder with `credential.seal` only leaves by `leave`, or after another device takes
  over and the hub answers that that identity was replaced, is already closed or is not known. If
  the hub says that a device whose folder has a credential was denied or expired, the connector
  discards nothing and shows the error: it may be an approved request that expired, a device
  expired by inactivity, or a sign of a hub restored from a backup.
- **Leave `~/.arauto` out of the backups of Hermes's volume.** Restoring an old copy of that
  folder takes the device to quarantine.

## 3. One leader per Hermes

Hermes starts the tool server as a child process, and more than one Hermes process can do that. If
each copy opened its own channel, the hub would see double connections of a healthy Hermes.

```
 copy 1  gets the lock    ->  LEADER: opens the channel with the hub and the local socket
 copy 2  does not get it  ->  RELAY: sends the calls to the leader through the socket
 copy 3  does not get it  ->  RELAY
 leader dies              ->  the system releases the lock; the next copy takes over
```

- Only the leader holds the channel, swaps the credential, tracks a new request and receives
  notices.
- The leader ends one connection attempt before opening another.
- For the hub there is exactly one channel connection per device. That is why "two connections"
  means a copy again. The short connections of `join` and of `leave` do not count.
- The local socket only accepts a process of the same user.
- Another local process of the same user can use the socket. That is the limit of quick mode; the
  record of each call stays in the audit of the hub.
- The folder of the connector cannot be shared between two Hermes, and the file system needs to
  support locks. Without that, two copies become leader, the hub treats them as a copy and the
  device goes to quarantine.
- A call in progress when the leader dies comes back with an error: AR-203 if it did not get to
  leave, AR-207 if it left and got no answer.
- When the hub tells the leader to stop using the channel (quarantine, suspended, revoked,
  incompatible), the leader stays alive: it answers the tools with the error and serves `join` and
  `leave` through the local socket.

**Waits of `await-reply` (proposal, 15/11/2026, [decision 0048](decisoes/0048-resposta-na-conversa-que-perguntou.md)).** The leader keeps the waits that `await-reply` registers, at most 8 per Hermes and one per connection. A closed connection frees its place, and normal waking applies again to that Hermes.

The format of the messages on the local socket is in
[14-protocolo-do-canal.md](14-protocolo-do-canal.md), section 11.

## 4. Registration in Hermes

The connector registers itself through the official Hermes command:

```
hermes mcp add arauto --command ~/.arauto/arauto-connector --env ARAUTO_HERMES_API_KEY=${API_SERVER_KEY} --args mcp --home ~/.arauto
```

(The line above is the shape of the command, written with the shell's `$`. The connector builds it
as separate arguments, with no shell, so the reference reaches Hermes as the literal text
`ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}`. That text is a fixed constant of the connector; nothing
from the hub, the invite or the command line enters it. It is not a secret.)

What the connector does (proposal until a trial on a real Hermes passes; the behavior of
Hermes below was checked on v0.22.5 on 14/11/2026, in
[hermes-fatos.md](../referencia/hermes-fatos.md)):

1. **Finds the program.** `ARAUTO_HERMES_BIN`, when set, is used exactly. Otherwise it tries
   `hermes` through the `PATH` and, only if the system says that program does not exist, on
   Unix, these fixed absolute places in order: `/opt/hermes/bin/hermes`,
   `/opt/hermes/.venv/bin/hermes` and `.local/bin/hermes` in the folder of the user (the last one
   also covers macOS). On Windows (change of 14/11/2026, approved) the list gains
   `<LOCALAPPDATA>\hermes\bin\hermes.exe`, with `LOCALAPPDATA` read from the environment of this
   machine and skipped when unset or not absolute. The refusal of a batch (`.cmd`) file stays: a
   Hermes whose launcher is only a `.cmd` is not found by this search and needs
   `ARAUTO_HERMES_BIN` set to a real program. The Windows place comes from the documentation of
   Hermes, read and not run ([hermes-fatos.md](../referencia/hermes-fatos.md)). The first
   that starts wins. Nothing from the hub or from the command line of `join` enters that list.
   If none exists, the failure says the program was not found and that `ARAUTO_HERMES_BIN` names
   it (AR-601).
2. **Answers the questions.** `hermes mcp add` asks on the terminal whether to overwrite an
   existing entry and whether to enable all the tools, and saves nothing if nobody answers, with
   the exit code 0 all the same. The connector gives it `y`, newline, `y`, newline on a pipe and
   closes the pipe. Its output is not read.
3. **Checks the list.** The command is only a success if a following `hermes mcp list` has a row
   whose first word is exactly `arauto` and that does not say `disabled`. The text of the list
   is read for this and for nothing else, up to 64 KiB: it is never logged, shown or put in an
   error.
4. **A disabled row is a failure.** If the row is there but disabled (Hermes could not probe the
   server and saved it anyway), the connector removes the entry, as far as it can, and fails
   (AR-601); the same `join` again tries again. A missing row also fails.

Not guaranteed: another version of Hermes may ask other questions, or print the list in another
format; the check depends on that format. In either case the registration fails (AR-601) and the
connector does not report one it could not see. The time limit of each run of the command is 90
seconds (the first run in a data folder Hermes never used took 47 s on 14/11/2026).

**The reference to the key (proposal, 15/11/2026; [decision 0046](decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md)).**
- The command is tried first with `--env ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}` (`--args` stays the
  last option). Hermes resolves the reference at spawn from its own secret scope and hands the value
  to the connector (observed on Hermes Agent v0.22.5, Docker install, Linux, 15/11/2026:
  [hermes-fatos.md](../referencia/hermes-fatos.md), K2 and K3).
- **Fallback.** If `hermes mcp add` exits with a non-zero code on that first try (a version that does
  not know `--env` rejects the option before asking anything), the connector runs it again without
  the option, the connection goes on, and `join` and `status` say that the entry has no reference and
  that no notice wakes this Hermes. A failure of another kind (list, time limit, program not found,
  disabled row) is not retried. Not covered: a Hermes that accepts `--env` and silently ignores it;
  then the entry line of `status` claims a reference that does nothing, and the key line, which asks
  the running connector, says "not received".
- **Environment of the Hermes command.** The connector gives the `hermes` command the environment of
  the terminal that runs it, minus `API_SERVER_KEY` and `ARAUTO_HERMES_API_KEY`. The command needs its
  own folders and `PATH`, so the rest is kept. Removing those two names means that, even if a launcher
  re-parsed its arguments through a shell, `${API_SERVER_KEY}` could expand to empty and not to the
  key. An empty value reaches the connector as no key.
- **A changed entry takes effect after the gateway restarts.** A running gateway did not apply an
  edited entry, nor restart a connector whose process died (observed in 15 minutes, K5). A new name
  registered from inside a session was picked up within a second. The connector does not restart
  Hermes; the `register` command therefore does not ask the leader to end.
- **Known limit of the connect-by-message path.** One Hermes refused the connect message, citing a
  local skill line that points to its `manage_connections` tool, which cannot add a local stdio
  server; the same command run by the operator in a terminal connected it (K9).
- Not proved: Windows (the `hermes.exe` launcher handing its command line to Python unchanged),
  macOS, v0.23 and other versions of Hermes; the Docker shim `/opt/hermes/bin/hermes` passing an
  argument that contains `${...}` unchanged. Nothing in the connector depends on a shell, but only a
  trial on each system shows the saved entry.

- The entry contains no secret (the reference `${API_SERVER_KEY}` names a variable and is not its value), and it is the only change that stays in the configuration of Hermes, apart from a key of the local API that the connector made for a Hermes that had none (section 6).
- Hermes checks every tool server entry when saving and when starting, and refuses the ones that
  look like an attack. The connector follows the format Hermes accepts: one program with one
  argument, with no command interpreter. The project does not try to get around that check.
- If Hermes asks for human approval to run the block or the steps of the instruction page, the one who approves is the operator. The
  flow works with or without that request.
- Hermes starts the connector by itself, in under a minute, without restarting, when the name is new. An entry that changes, or a connector that dies, waits for the gateway to restart (see above).

## 5. Startup and shutdown

**Startup with the hub down.** The connector does not die: it answers Hermes with the tool list of
the last session and returns the error AR-203 on the calls. It tries the hub again with a growing
and random wait, from 1 to 60 seconds.

**Shutdown.** Hermes closes the input of the connector and waits 2 seconds before ending it by
force. In that time the connector closes the channel in order. There is nothing to save on the way
out: the credential has been on disk since the last swap.

## 6. Waking Hermes

On receiving a notice, the leader calls Hermes's local API with the fixed sentence
([03-arquitetura.md](03-arquitetura.md), section 7) and only then confirms to the hub. A repeated
notice is discarded by its identifier. The connector accepts at most 30 wakes per minute and 300 per
hour, adding up the kinds, so that a faulty hub does not trigger runs without stopping. Before
building the sentence it checks the format of each field of the notice
([14](14-protocolo-do-canal.md), section 5.3).

**The key of the local API.** Hermes's API requires a key, which Hermes keeps in its own setting
`API_SERVER_KEY`. The API listens only when that setting holds a key of at least 16 characters
(observed, K1). The connector, as a child process, does not inherit Hermes's environment. There were
three paths ([decisoes/0023](decisoes/0023-chave-da-api-local.md)); state on 15/11/2026:

| Path | State |
|---|---|
| The server entry references Hermes's variable | **Observed on one Hermes** (Hermes Agent v0.22.5, Docker install, Linux, 15/11/2026): the entry carries `ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}`, Hermes hands the resolved value to the connector, and a note on a link with the wake mark woke that Hermes without a person talking to it (K2 to K4). This is what `join` and `register` save (section 4). **Not checked** on Windows, macOS, v0.23 or other versions. |
| The connector reads the key from Hermes's secrets file | **Not built** and not needed for now. |
| No key | **Option B** (proposal, built on 16/11/2026, rehearsed once on 16/11/2026, on Hermes Agent v0.23.2 from the official Docker image): `join` makes one when the conditions below hold. Otherwise **Option A** (proposal, [decision 0046](decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md)): see below. |

**A Hermes without a local API key (Option A, proposal).** The connector still connects. At `join`,
at `register` and in `status` it says plainly that this Hermes cannot be woken until its local API is
on, names Hermes's own setting `API_SERVER_KEY` and says that it is set in the configuration of
Hermes (when the entry carries the reference; otherwise it says that the reference was refused). **The connector never writes Hermes's secrets file itself and never restarts Hermes.** `status` is the authority on whether the key reached the connector.

**A Hermes with no key (Option B, proposal, built on 16/11/2026, rehearsed once on 16/11/2026, on Hermes Agent v0.23.2 from the official Docker image; [decision 0046](decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md)).**

1. `hermes config get API_SERVER_KEY`. Exit 0: a key is set; stop, print nothing more. Exit 1: go on. Anything else (another code, not started, killed): **KeyNotMade**.
2. **Listener guard.** A TCP connection to `127.0.0.1:8642`, at most 2 s, nothing sent, closed at once. Accepted: **KeyNotMade**. Refused or no answer: go on.
3. **Address guard.** `hermes config get` on `API_SERVER_HOST`, `gateway.platforms.api_server`, `gateway.api_server.host`, `gateway.api_server.extra`, in this order. Each must exit 1; the first that does not: **KeyNotMade**.
4. 32 bytes from `getrandom::fill`, written as 64 lowercase hexadecimal characters in a `Zeroizing<String>`. Failure: **KeyNotMade**.
5. `hermes config set API_SERVER_KEY <key>`. Exit 0: **KeyMade**, then **RestartAfterKey**. Anything else, killed included: **KeyNotMade**.
6. The key is wiped from memory. The exit code of `join` stays what it was.

| Stops at | Printed after the code | Changed in Hermes |
|---|---|---|
| 1, exit 0 | nothing new | nothing |
| 1 other, 2, 3, 4 | KeyNotMade | nothing |
| 5, exit 0 | KeyMade, RestartAfterKey | one line `API_SERVER_KEY=` in `<HERMES_HOME>/.env` (N13) |
| 5, other or killed | KeyNotMade | unknown: a killed `set` may have written; the next `join` finds it at step 1 |

`status` asks the **leader** over the local socket and never reads its own environment for this line.
The leader sends `waking`: whether the running connector received the key, and the last wake
outcome. It prints:

| Line | Values |
|---|---|
| `Entry in Hermes` | saved with a reference to `API_SERVER_KEY`; saved without a reference to the key (this version of Hermes refused it); saved by an older connector, perhaps without a reference; not saved by this connector. |
| `Key of the local API of Hermes` | received by the running connector; not received by the running connector (no notice wakes this Hermes); unknown (the running connector is of an older version, or no connector started by Hermes is running). |
| `Last wake` | accepted by the local API of Hermes; refused by the local API of Hermes; the local API of Hermes did not answer; none since the connector started. |

The line no longer depends on the state of the channel. After a refusal or no answer the connector
also prints the AR-602 phrase, and logs a warning that carries neither the key nor the body of the
answer. `arauto_status` (the tool) carries one more field, `local_waking`, added by the connector: the
connector of this Hermes received the key of the local API; when `false`, nothing wakes this Hermes
whatever `can_be_woken` says ([09](09-contrato-mcp.md), [decision 0046](decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md)). (16/11/2026, [decision 0049](decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md): `can_be_woken` now means notes or the verb "be woken"; the switch it once followed is gone.)

`arauto_status` carries one more field, `keep_in_memory`, also added by the connector (proposal, 15/11/2026, [decision 0047](decisoes/0047-notas-do-arauto-na-memoria.md)): a reminder sentence followed by the three Arauto lines that a Hermes keeps in its own main memory. It is fixed text, added only to a successful answer, and it overwrites any value of the hub. The connector never reads or writes the memory or the files of Hermes (principle 7), apart from the key of the local API that Hermes's own command saves in `join` (section 6): the Hermes saves the lines with its own tool, and the connector cannot know whether it did.

**A failed wake.** The connector answers the hub `failed`, and the hub puts the notice back to pending
with waits of 5 s, 10 s, 20 s, up to 5 minutes, until it expires. The note itself is kept and
readable. A Hermes without a key therefore keeps bouncing its notices every few minutes until they
expire (seen on the sender side on 15/11/2026, K4); the hub does not change.

**The call to the API.** Observed on one Hermes (v0.22.5, 15/11/2026): the call with the fixed
sentence was accepted, and the log of that Hermes shows an inbound run on its API server (K4).
Not separately checked: the use of the API's repetition identifier, an API on another address or
port than `127.0.0.1:8642` (not followed; only `ARAUTO_HERMES_API_URL` set by hand), and any other
Hermes version or system.

### An answer in the conversation that asked (proposal, decision 0048)

**Proposal, 15/11/2026. Built and covered by the automatic tests; not tried on a real Hermes.** Context: a woken Hermes wrote its answer into the run instead of sending a note, and an asking Hermes ended its turn, so a later answer woke it in a separate run that its operator never saw ([hermes-fatos.md](../referencia/hermes-fatos.md), N6 and N7; the background process with notify is N2 and N5).

- **The field `to_get_the_reply`.** The connector adds it to a successful `hermes_send_message`, only when the thread of the hub parses as a UUID, and prints the thread hyphenated. It overwrites any value of the hub. Three fixed lines tell the asking Hermes to run `"$HOME/.arauto/arauto-connector" await-reply THREAD` as a background process with `notify` on, tell its operator, and end its turn ([09](09-contrato-mcp.md)).
- **The wait.** The leader lists the threads once at registration (a listing never marks a note read), then every 30 seconds, one listing for all waiters. An answer is unread notes in the thread, or more unread notes than the baseline in another thread of the same peer.
- **The notice path.** When a peer notice arrives from a peer that someone awaits, the leader lists at once. If a waiter resolves, the leader does **not** call the local API: it answers `accepted` and records the notice identifier as handled. No place is taken in the wake ceiling. A listing that fails falls through to the normal wake.
- **The grace check.** Ten minutes after a waiter was resolved by a notice, the leader lists again. If the thread still has unread notes, it wakes through the local API with the normal peer sentence, the identifier of the stored notice as the repetition key and the normal windows. A restart of the leader loses it: the grace checks are kept in memory only, so a Hermes that never read the answer is then not woken for it.
- **A resolution by polling alone never wakes.** It makes no call to the local API, at once or later. A link without the wake mark therefore makes no call at all.
- **The grace check uses the first (data) sentence, never the follow sentence** ([decision 0050](decisoes/0050-etapa-b-marca-de-seguir.md)): it wakes a Hermes for an answer that a waiter announced and needs no authority.
- **A notice of another peer, and a WhatsApp notice, wake as before.**
- **`accepted` now means** that the Hermes was told, by its local API or by a waiting `await-reply` ([14](14-protocolo-do-canal.md), section 5.3).
- **The peer sentence** is new: it says that what a woken run writes reaches no one and how to answer ([13](13-conversa-entre-hermes.md), section 5). It rests on the change of 15/11/2026 to [decision 0005](decisoes/0005-comando-e-dado-sao-separados.md), approved by the maintainer.
- **Not guaranteed:** the list in [decision 0048](decisoes/0048-resposta-na-conversa-que-perguntou.md): among others, a reply after `/new`, after the deadline or after a restart does not appear in the conversation; a woken Hermes cannot use `notify`; whether a model follows the field is untried; compression may kill the wait; Windows is untried. A poll can resolve a wait just before the notice of the same answer arrives; the notice then finds no wait and wakes a separate run too, so the Hermes may be told twice (the identifier keeps the wake itself to one). The same happens when the answer arrives before `await-reply` registers: its notice wakes a separate run and the wait then resolves at once. The grace check looks at every thread of that peer, not only at the awaited one, so a later note of the same peer in another thread may cause a second wake. The wake of the grace check is not made when the connector has no key of the local API, and one that fails is not tried again.

### The follow sentence (proposal, decision 0050)

**Proposal, 16/11/2026. The field and the second fixed sentence are built in the protocol crate and the connector, with their tests; not tried on a real Hermes.** A peer notice with `follow: true` makes the leader start the run with a second fixed sentence, approved word for word by the maintainer (text in [0050](decisoes/0050-etapa-b-marca-de-seguir.md), section C; [13](13-conversa-entre-hermes.md), section 5). It asks the receiver, in that run only, to do what the new messages of that notice ask without waiting for its operator, unless its operator's instructions forbid it. A notice with `follow` absent, `null` or `false` gets the first sentence; any other value is `rejected` ([14](14-protocolo-do-canal.md), section 5.3). The connector does not execute anything from the hub: it only chooses which of two sentences compiled into it to show. **Not guaranteed:** that a model follows the sentence or refuses when its operator forbids it; an invaded hub can make any peer notice carry the field and the connector cannot tell. The descriptions of the tools do not change.

### Not guaranteed (waking and the key)

- Only one Hermes was observed. The reference, the resolution of `${VAR}` and the wake on Windows,
  macOS and other versions are unproved.
- The key now sits in the initial environment of every connector copy. On Linux, any process of the
  same user, including the LLM's terminal, can read `/proc/PID/environ` without the approval Hermes
  asks before reading its secrets file. Hermes already does the same for the provider keys it
  injects (K6). Whether this is acceptable under the security principles is for the maintainer.
- The connector reads only its own variables (`ARAUTO_HOME`, `ARAUTO_HERMES_BIN`,
  `ARAUTO_HERMES_API_URL`, `ARAUTO_HERMES_API_KEY`, and the system location variables for the
  default folder and for the place of Hermes), and the key only in the `mcp` process. A test of the
  source checks that no other read appears; it is a check of the text, not of the running program.
- A newer entry is not used until the gateway restarts, and a copy started from a newer entry while
  an older leader lives does not wake: the leader decides.
- The connector never reads or writes the memory of Hermes and cannot check whether the Hermes saved the three Arauto notes (proposal, 15/11/2026, [decision 0047](decisoes/0047-notas-do-arauto-na-memoria.md)). `keep_in_memory` and the line printed by `leave` are reminders, not checks.
- Hermes masks printed output by variable name; whether it hides `API_SERVER_KEY` in the lines the
  connector prints was not tried.
- **The key in the arguments of one process**, while `config set` runs: on Linux every local user can read it unless `/proc` is mounted with `hidepid`; on Windows at least the same user and the administrators; macOS not checked. If Hermes's launcher starts a second process for the command, that one too (not checked). Whether Hermes logs its own arguments: not checked (rehearsal still owed).
- **Where the key rests afterwards:** `.env` in plain text with Hermes's rights (mode 600 on the rehearsed Hermes, N13; Windows: the folder's ACL, not checked); the gateway's memory; the environment of every connector copy, as today (K3); Hermes's dashboard lists the setting (`hermes_cli/web_server_messaging.py` 31 [code]), masked or not, not checked. Arauto keeps no copy, no digest and no file of it.
- **The guards see what the terminal sees.** A key or an `API_SERVER_HOST` that lives only in the gateway's process environment is invisible (N12). The listener guard catches the key only while something answers on `127.0.0.1:8642`. Not covered: such a key with another port, a non-wildcard non-loopback host, an IPv6 wildcard `::` that is v6-only (the Windows default), or with the gateway down during `join`; then the new key replaces the operator's at the next restart (code of Hermes, read, not run) and their clients are refused. Not covered either: such an `API_SERVER_HOST` naming a network address with no key anywhere; then the next restart opens the API, with the new key, on that address. A host in a legacy `gateway.json` is not checked.
- **No key, even when harmless:** any host setting, an explicit `127.0.0.1` included, anything on port 8642 (another Hermes, another program, the default listener of another profile). The operator then sets the key by hand.
- **The key stays** after a `join` later denied or expired, after `leave`, and after a `join` to a hub that was not the operator's. Nothing removes it.
- **It takes effect at the next restart of the gateway, by anyone:** the operator's `/restart` (or, where the chat refuses it, the Restart Gateway button of the Hermes dashboard), a reboot or an update. Until then nothing wakes. A Hermes without a gateway has no local API at all.
- **Hostile text.** A note can lead the Hermes to run an old block again (AR-104) or a block of another hub. At most a key is made where none was and nothing listened, never sent anywhere; the loopback API then turns on at the next restart.
- **Exit codes only.** An empty `API_SERVER_KEY=` line counts as set: no key is made and nothing more is printed. A crash of the `hermes` command that exits 1 reads as "not set" at steps 1 and 3.

## 7. Versions and distribution

- Each version of the connector is published in the project's releases, signed with the release
  key, with the SHA-256 digest beside it.
- Each version of the hub brings the connectors compatible with it and serves them at the download
  address.
- **In every installation through the block**, the first and each reconnect, the check is the
  SHA-256 digest that comes in it. Whoever wants a second source compares it with the digest
  published in the project's releases.
- The signature serves for the hub to check the connectors it distributes and for whoever wants to
  check from outside. If one day the connector updates itself, it will only accept a signed version
  with a number higher than its own.
- The hub requires a minimum version. Below it, every tool answers AR-401.
- The first message of the channel never changes format, so that a very old connector still
  receives AR-401 instead of a mute error. What else stays frozen is in
  [14-protocolo-do-canal.md](14-protocolo-do-canal.md), section 5.1.
- Since 18/11/2026 (decision 0051, proposal) the channel protocol is at version 2 and both programs speak versions 1 to 2: a connector installed before keeps working, and gets AR-705/AR-712 where a newer one gets AR-714, until the admin reconnects that Hermes.
- In the first version of the project, updating is **reconnecting**: new block, accesses kept. There
  is no update pushed by the hub.

## 8. Message catalog

Every message is fixed text, with no accents, and says what happened and who solves it. None
carries a command to run. The text `keep_in_memory` of `arauto_status` is not a catalog message: it repeats the three approved lines, the third of which names a command (proposal, [decision 0047](decisoes/0047-notas-do-arauto-na-memoria.md)).

| Code | When |
|---|---|
| AR-101 | Invite not accepted: expired, cancelled, nonexistent or already used. |
| AR-102 | The invite was already used by another device. Nothing was installed here. |
| AR-104 | This request was already approved. Nothing to do. |
| AR-105 | There is already in this folder a device of another hub. It is necessary to leave it first; if the old hub no longer exists or changed its key, a person does the local leave. |
| AR-201 | No connection with the hub, with the cause: name not resolved, timeout, connection refused. |
| AR-202 | The hub is not the one of the key that came in the block. Connection refused. |
| AR-203 | Channel down: the call did not leave, and can be repeated. The connector keeps trying. |
| AR-204 | The hub closed the connection: protocol error or limit. The connector tries again. |
| AR-205 | The hub is stopping or out of resources. The connector tries again in a moment. |
| AR-206 | This attempt was swapped for a newer one of the same device. |
| AR-207 | The call left and the channel dropped before the answer: the result is unknown. Check before repeating. |
| AR-208 | Connection taken for dead: another connection of the same device came in its place. The connector reconnects at once. |
| AR-301 | The request was denied. |
| AR-302 | The request expired without approval: no one confirmed the code in time, or the hub closed the request. |
| AR-303 | The request was approved, but expired before the connector concluded. The admin needs to reconnect. |
| AR-401 | Connector out of date. The admin needs to reconnect this Hermes through the panel. |
| AR-402 | The hub is older than this connector. Whoever installs needs to update the hub. |
| AR-501 | Device in quarantine: the hub no longer trusts the credential. The admin needs to reconnect this Hermes through the panel. |
| AR-502 | Device revoked. |
| AR-503 | Device replaced by another. |
| AR-504 | The hub does not recognize this device. Nothing was deleted. |
| AR-505 | Leave pending: the hub did not answer. The device needs to be revoked in the panel. |
| AR-506 | Device suspended: the hub came back from a backup. The admin needs to reconnect this Hermes through the panel. |
| AR-507 | Device expired by inactivity. The admin needs to reconnect this Hermes. |
| AR-601 | The registration in Hermes failed or was refused. The request still stands; run the command again. |
| AR-602 | Hermes's local API does not answer or refuses the key. |
| `await-reply` lines | Proposal, 15/11/2026 ([decision 0048](decisoes/0048-resposta-na-conversa-que-perguntou.md)). Fixed lines of the command, with only a validated thread, a validated name and the minutes. They are not catalog messages: the line of exit 0 is longer than the 240 bytes of the catalog rule, and the field `to_get_the_reply` names a command, like `keep_in_memory`. The refusal line of exit 12 carries the code and the sentence of the catalog. |
| Told lines | Fixed sentences that `join`, `register` and `status` print about waking (proposal, 15/11/2026; decision 0046). Wake needs the local API: "This Hermes can be woken only while its local API is on, which Hermes does when its own setting API_SERVER_KEY holds a key. The status command says whether the connector received it." Key not received: "No notice wakes this Hermes until its local API is on. Hermes turns it on when its own setting API_SERVER_KEY, kept in the configuration of Hermes, holds a key." Key made: "This Hermes had no key for its local API, so the connector drew one and saved it in API_SERVER_KEY with the config command of Hermes. The connector shows it to nobody and writes it in no file of its own." Restart after the key: "The local API of this Hermes turns on only after its gateway restarts. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard." Key not made: "The connector did not confirm a new key for the local API of this Hermes. If the status command says the key is not received, the operator sets API_SERVER_KEY in the configuration of Hermes and restarts its gateway." After a restart: "If that setting already holds a key, Hermes hands it to the connector after it restarts its gateway." Reference refused: "This version of Hermes refused the reference to the key of its local API, so the entry was saved without it and no notice wakes this Hermes. Notes and tools still work." Running keeps the old entry: "A running Hermes keeps the entry it loaded: the new entry takes effect after Hermes restarts its gateway. The connector does not restart Hermes." Older entry: "The entry of the connector in Hermes was saved by an older connector and may have no reference to the key. The register command of the connector saves it again." Unchanged connection: "The connection did not change: the same device and the same credential." |
| No device (`register`) | "There is no device in this folder. Nothing was registered in Hermes." Exit code 9, like the other local failures. |
| Notes in memory (`leave`) | Proposal, 15/11/2026 ([decision 0047](decisoes/0047-notas-do-arauto-na-memoria.md)). Printed by `leave` after a leave that deleted the files: "If this Hermes saved the three Arauto notes in its memory, they are still there: remove them with the memory tool of Hermes. The connector never reads or changes the memory of Hermes." |
| AR-701 | The verb needed for this call is missing. |
| AR-702 | Number unavailable: disconnected from WhatsApp or waiting for the admin's confirmation. |
| AR-703 | Engine unavailable. |
| AR-704 | Send ceiling reached. |
| AR-705 | Chat not found. It is the same answer for a chat that does not exist and for a chat out of scope. |
| AR-706 | Note ceiling reached. The message says in how long it comes back; insisting does not help. |
| AR-707 | There is no peer with that name that this Hermes can send to. Only the admin changes that. |
| AR-708 | Thread not found. It is the same answer for a thread that does not exist and for a thread of others. |
| AR-709 | Too large: ask for fewer items or send smaller content. |
| AR-710 | Too many calls at the same time. Try again in a moment. |
| AR-711 | Invalid argument. |
| AR-712 | Message not found: it does not exist, is outside what this Hermes can use, or WhatsApp no longer allows it. |
| AR-713 | The result of the sending is unknown: the message may have been sent. Read the chat before sending it again. |
| AR-714 | (18/11/2026, decision 0051, proposal) The chat is the command channel of this Hermes, as the admin recorded it: Arauto does not write in it, and the Hermes replies through its own WhatsApp. Sent only on a connection of protocol version 2; every other Hermes gets AR-705 or AR-712. |

The sentence of each code, in English, is compiled into the connector.

## 9. Shielded mode (planned)

In quick mode, described here, the identity and the credential stay where the LLM's terminal
reaches. In shielded mode, a person installs the connector as a separate service, with another user
or in another container, and Hermes talks to it through a local address. The LLM no longer reaches
the files.

It is not for the first version. The panel shows, per Hermes, the limit of the mode in use: "Basic
protection: the LLM reaches the credential file" or "Shielded protection".

## 10. Platforms

| Platform | When |
|---|---|
| Linux x86_64 | First. |
| Linux arm64 | Later. |
| Windows | Development ([decision 0036](decisoes/0036-windows-so-para-desenvolvimento.md)). Since 14/11/2026 the image also builds a Windows x86_64 file (see below); it is served, not a supported platform, and the limits of 0036 still hold. |
| macOS | Later for use. Since 14/11/2026 the image also builds macOS arm64 and x86_64 files (see below). |

**Compiled, never run (change of 14/11/2026, facts stated by the maintainer's assistant).** In the
build image (`rust:1.98-bookworm`) the connector, with its `tls` feature, compiles and links for
`x86_64-pc-windows-gnu` (with `gcc-mingw-w64-x86-64`, about 13 MB) and for `aarch64-apple-darwin`
and `x86_64-apple-darwin` (with `cargo-zigbuild` and `ziglang`, about 7.5 MB each; the linker warns
that no Apple SDK was found and still links; the arm64 file carries an ad hoc code signature
command). **None of these three files was ever executed.** The code of the connector runs on a
Windows machine in the test suite, but with the MSVC toolchain, not this GNU build. There is no Mac
available. Nothing here says the Windows or macOS connector works.
