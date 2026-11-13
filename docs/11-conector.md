# 11 — The connector

**Status: proposal.** The program that runs beside Hermes.

## Summary

```
arauto-connector join     asks the hub for a connection and registers itself in Hermes
arauto-connector mcp      the tool server that Hermes starts
arauto-connector status   shows the situation, with no secret
arauto-connector leave    asks the hub for its own closing and cleans the folder
                          (leave --local: cleans without the hub; it is for a person)
```

One file only, with nothing else to install. It opens no network port, runs no command from the hub
and shows no secret. The only thing it discards on information from the hub is a connection request
that has not received a credential yet.

## 1. Commands

| Command | What it does |
|---|---|
| `join HUB KEY INVITE` | Creates the folder of the request with a new identity and does the enrollment with the hub of that key. If the hub accepts, it copies itself to the fixed path `~/.arauto/arauto-connector` (through a temporary file and a rename, because the old program may be running), registers the tool server in Hermes and shows the verification code. It ends in seconds. With `--no-register`, for development, it skips the registration. |
| `mcp` | What Hermes starts. It speaks MCP over stdio with Hermes and keeps the channel with the hub. |
| `status` | Hub, version, situation of the device, whether the Hermes can be woken. It reads the local files and asks the leader through the local socket. It does not open a connection of its own with the hub. |
| `leave` | Asks the hub for the closing of its own device. With the answer, it deletes the files and removes the registration in Hermes. |
| `leave --local` | Deletes the files and the registration without talking to the hub. It exists for when the hub no longer exists or changed its key. It is for a person to run: the device stays valid in the hub until it is revoked in the panel. |

### Cases of `join`

| Situation | What happens |
|---|---|
| Same invite again, request still pending | It is the same request, with the same identity: the connector recognizes the invite by the digest kept in the folder of the request. The verification code is another one, and the most recent is the valid one. The hub accepts up to five enrollments per request. |
| Same invite again, request already approved | The hub answers AR-104. Nothing changes: the connector only redoes the registration in Hermes, if it is missing. |
| Same invite again, request denied or expired | The hub answers AR-301, AR-302 or AR-303. The connector discards the folder of that request only if it has no `credential.seal` yet; if it has one, it discards nothing and shows the error. |
| The hub refuses the invite (AR-101, AR-102) | It discards the folder it has just created for this request. Nothing else changes on the machine. |
| New invite, with an old request with no credential sitting in the folder | It discards the old request and creates another, with a new identity. |
| The enrollment worked and the registration in Hermes failed | It shows AR-601. The request still stands in the hub; running it again tries the registration. |
| There is already a device in the folder, of the same hub | It is treated as a reconnect, whatever the line of the invite in the panel. The current device keeps working until the new one has its first credential confirmed. When the new one takes over, the connector asks for the closing of the old one, if the hub has not yet given it as replaced, and of every other identity that is left in the folder; "already closed" and "I do not know this device" count as confirmation. Each folder is only deleted after the answer of the hub; until then, the leader repeats the request at every startup and every time the channel comes back. |
| There is already a device in the folder, of another hub | It refuses (AR-105). Only after `leave`; if the old hub no longer exists or changed its key, after `leave --local`. The same address with another key counts as another hub. |

At the end, `join` tells the running leader, through the local socket, that there is a new request.
If the program was swapped for a new version, it asks the leader to exit, so that Hermes starts it
again already in the new version. This depends on a proof: Hermes restarts a tool server that
exits ([12-roteiro.md](12-roteiro.md), Phase 1). If it does not restart it, the alternative is for
the leader to swap itself for the new program, without closing the connection with Hermes.

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
another folder; in that case the registration in Hermes carries `--home` in the arguments
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
                              request, local situation
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

The format of the messages on the local socket is in
[14-protocolo-do-canal.md](14-protocolo-do-canal.md), section 11.

## 4. Registration in Hermes

The connector registers itself through the official Hermes command:

```
hermes mcp add arauto --command ~/.arauto/arauto-connector --args mcp
```

- The entry contains no secret, and it is the only change that stays in the configuration of Hermes.
- Hermes checks every tool server entry when saving and when starting, and refuses the ones that
  look like an attack. The connector follows the format Hermes accepts: one program with one
  argument, with no command interpreter. The project does not try to get around that check.
- If Hermes asks for human approval to run the block, the one who approves is the operator. The
  flow works with or without that request.
- Hermes starts the connector by itself, in under a minute, without restarting.

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

**The key of the local API.** Hermes's API requires a key, which stays in Hermes's secrets file.
The connector, as a child process, does not receive it from the environment. There are three paths,
and the order is decided ([decisoes/0023](decisoes/0023-chave-da-api-local.md)): the first is tried;
if the test of Phase 1 shows that it does not work, the second applies; with no key, the third is
left.

| Path | Pros and cons |
|---|---|
| The server entry references Hermes's variable | Hermes itself hands the key to the child. Nothing is copied. It depends on Hermes accepting the reference in that field. |
| The connector reads the key from Hermes's secrets file | It always works. An outside program reading that file needs to be told to whoever installs. |
| No key, no waking | Hermes uses the tools when asked and is never woken. The panel shows "Can be woken: no". |

Address, body of the call and the use of the API's repetition identifier are deliverables of Phase 1.

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
- In the first version of the project, updating is **reconnecting**: new block, accesses kept. There
  is no update pushed by the hub.

## 8. Message catalog

Every message is fixed text, with no accents, and says what happened and who solves it. None
carries a command to run.

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
| Windows | Only for development ([decision 0036](decisoes/0036-windows-so-para-desenvolvimento.md)). |
| macOS | Later. |
