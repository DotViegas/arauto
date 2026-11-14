# 10 — State machines

**Status: proposal.** The states of each thing, who changes each one and the deadlines. The numbers
are starting points, to be calibrated in the prototype.

## Summary

```
INVITE      issued -> used | expired | cancelled
DEVICE      pending -> approved -> active
            exits: denied, expired, quarantined, suspended, replaced, revoked
NUMBER      unpaired -> pairing -> connected <-> reconnecting
            detours: disconnected, number swapped, failed, removed
NOTICE      pending -> sent -> confirmed
            exits: expired, discarded
```

Every deadline uses only the clock of the hub.

## 1. Invite

| From | To | Who triggers it |
|---|---|---|
| (nothing) | **issued** | The admin clicks "Connect Hermes" or "Reconnect". |
| issued | **used** | First valid enrollment. |
| issued | **expired** | 15 minutes with no enrollment. The hub deletes the digest of the invite. |
| issued | **cancelled** | The admin cancels, generates another invite for the same Hermes or revokes the Hermes; or the hub enters restored mode. |

- A Hermes has at most one issued invite at a time.
- Final states stay recorded. An invite never becomes valid again.
- In the hub, the digest is also deleted when the invite is cancelled, and not only when it expires
  ([14](14-protocolo-do-canal.md), section 12). An expired invite that an enrollment finds is
  closed right there.
- **The instruction page of the invite** ([14](14-protocolo-do-canal.md), section 1; proposal, not tried
  on a real Hermes) serves while the invite is **issued** and not expired, or **used** and its device
  is **pending** before the verification deadline. It stops, and its digest is erased in the same
  write, when the invite is cancelled or expires, and when the request is approved, denied, expires
  or is closed. A new invite for the same Hermes does either of those to the old one. Reading the
  page never changes a state.
- A used invite, presented by **another identity** while the request is pending: refusal and an
  **alert** in the panel.
- In any other case of an invite that is not valid: refusal and only a record in the audit. **It
  never brings down a connected Hermes.**

## 2. Device

| From | To | Who triggers it |
|---|---|---|
| (nothing) | **pending** | Valid enrollment ([04](04-seguranca-conexao.md), C3). |
| pending | **approved** | The admin types the right verification code. |
| pending | **denied** | Five wrong codes; the sixth enrollment; the admin denies, generates another invite for the same Hermes or revokes the Hermes; the device itself gives up (`leave`); or two waiting connections with the same identity. |
| pending | **expired** | 10 minutes with no code, counted from the first enrollment; or the hub enters restored mode. |
| approved | **active** | The first credential was confirmed. |
| approved | **expired** | 24 hours without confirming the first credential; or the hub enters restored mode. |
| approved, active | **quarantined** | The hub no longer trusts the credential: reuse, double connection or unreadable credential; or the admin does not recognize the last use of an active device when approving a reconnect. |
| active | **suspended** | The hub entered restored mode ([04](04-seguranca-conexao.md), C11). |
| active | **expired** | 30 days without connecting ([decisoes/0021](decisoes/0021-prazo-de-inatividade.md)). |
| active, suspended, quarantined | **replaced** | A new device of the same Hermes had its first credential confirmed (Reconnect). |
| approved, active, suspended, quarantined | **revoked** | The admin revokes the Hermes or deletes it; the admin generates another invite and the device did not have a confirmed credential yet; or the device itself asks for its closing (`leave`, or when it is swapped for another on the same machine). A closing asked by the device generates an alert. |

- A Hermes has at most one device in use (the one that has already confirmed a credential: active,
  suspended or in quarantine) and one request in progress (the one that has not confirmed yet:
  pending or approved, even if it went to quarantine).
- Denied, expired, replaced and revoked are final. The line stays, as a record: if the same
  identity tries to come back, the hub recognizes it and answers its situation.
- Out of quarantine and out of suspended there is only Reconnect (it becomes replaced) or Revoke.
  There is no "release" and no "reactivate".
- "Incompatible" is not a state of the device. It is a label computed from the version of the
  connector recorded at the last accepted connection.

In the hub, on 11/11/2026:

- **An expired deadline applies at once.** Whoever finds an expired deadline before the sweep
  applies it there: the enrollment, the approval and the two writes of the swap. Those of the swap
  also apply the 30 days of inactivity.
- **Denying** through the admin command only applies to a pending request. An approved one with no
  credential leaves by revoke or by a new invite.
- **Deleting a Hermes** closes everything that is its own, like a revoke, and removes its links
  ([13](13-conversa-entre-hermes.md), section 7).
- **Expired with no reason that explains it** answers AR-507, the only code that never leads the
  connector to discard anything.

## 3. Hermes: what the screen shows

The label of the line is computed from the device and the channel.

| Label | When |
|---|---|
| No device | The line exists, with no issued invite and no live device. |
| Waiting for the Hermes | There is an issued invite. |
| Waiting for your code | There is a pending device. |
| Approved, waiting for the connector | Device approved, still without the first credential confirmed. |
| Connected | Active device with an open channel session. |
| Disconnected for X | Active device with no channel session. |
| Incompatible | The connector is below the minimum version. |
| In quarantine | The hub no longer trusts the credential. |
| Suspended | The hub came back from a backup. This Hermes needs to be reconnected. |
| Revoked | The device was closed. |

The accesses belong to the line of the Hermes and apply to the active device. Changing devices does
not touch them. With the device in any other state, every call is denied.

The label of the device expired by inactivity is not written yet
([decisoes/0021](decisoes/0021-prazo-de-inatividade.md)). By the rules above, the line appears as
"No device".

## 4. Number

| From | To | Who triggers it |
|---|---|---|
| (nothing) | **unpaired** | The admin adds the number. |
| unpaired, disconnected | **pairing** | The admin asks for the pairing code or the QR. |
| pairing | **connected** | The phone confirmed, and the paired number is the expected one. |
| pairing | **number swapped** | The phone confirmed, but the paired number is another one. |
| pairing | **unpaired** | The pairing code or the QR expired unused. |
| connected | **reconnecting** | The engine lost the connection with WhatsApp. |
| reconnecting | **connected** | The connection came back. |
| connected, reconnecting | **disconnected** | Someone removed the linked device, on the phone, or the admin disconnected through the panel. |
| number swapped | **connected** | The admin confirms the new number. |
| number swapped | **unpaired** | The admin undoes the pairing. |
| connected, reconnecting | **failed** | The engine reported an error in the engine session. |
| failed | **reconnecting** | The admin restarts the engine session. |
| any | **removed** | The admin removes the number. |

- In **number swapped** the accesses stay stopped until the admin decides.
- **Unknown state** is not a state of the number: it is what the screen shows when the engine does
  not answer.
- The hub checks the configuration of each engine session at startup and every 5 minutes. If it
  finds something different from what is expected (for example, an event destination that is not
  the hub), it corrects it and alerts.

## 5. Notice

| From | To | Who triggers it |
|---|---|---|
| (nothing) | **pending** | A message arrived in a chat in which the Hermes may be woken, or a note from a peer that wakes it ([13](13-conversa-entre-hermes.md)). |
| pending | **sent** | The hub delivered it to the connector. |
| sent | **confirmed** | The connector confirmed after Hermes's local API accepted. |
| sent | **pending** | The connector did not confirm in time, or answered that it failed or that it reached its ceiling; the hub tries again. |
| pending, sent | **expired** | 24 hours in the queue, or the queue of the Hermes went past 1,000 and this was the oldest. |
| pending, sent | **discarded** | The access or the link left before the confirmation, or the connector refused the notice for being out of format. |

- Delivery is at least once. The connector discards a repeated notice by its identifier.
- There is only one pending notice per target (the chat, or the sender Hermes): what arrives later
  adds to the count. After a delivered notice, the next one for the same target only leaves in 10
  seconds.
- A message sent by the number itself wakes no one.

## 6. Connection and channel session

A channel connection starts at the handshake. The **channel session** starts when the swap of the
credential is confirmed ([04](04-seguranca-conexao.md), C7). Before that the connection only serves
for the waiting of the request or for the swap itself. The short connections of `join` and of
`leave` are not channel connections ([04](04-seguranca-conexao.md), C9).

| Reason for the end | What the connector does |
|---|---|
| Network drop, or missing heartbeat | Reconnects at once. If the attempt fails, it opens another, with a growing and random wait, from 1 to 60 seconds. |
| Taken for dead (the hub accepted another connection in its place; AR-208) | Reconnects at once. If someone took its place, that is where it shows. |
| The hub is stopping or out of resources (AR-205) | Reconnects after a random wait. |
| Attempt swapped for a newer one of the same device (AR-206) | Waits from 5 to 15 seconds, drawn at random, and tries again. |
| Protocol error or limit (AR-204) | Reconnects with a growing wait and does not repeat the same message. |
| The "saved" of the 24-hour swap did not arrive in 60 seconds | Reconnects and presents the credential it has. |
| Device replaced (AR-503) | Starts using the folder of the new device. If it has no new folder, it stops using the channel and shows the error. |
| Request denied or expired | If the folder of that request has no credential yet, it discards the folder and, if there is no other device, removes the registration. If it has one, it discards nothing and shows the error. |
| Incompatible | Stops using the channel and shows the version error in every tool. |
| Quarantine, suspended | Stops using the channel and shows the error. It deletes nothing. |
| Revoked | Stops using the channel and shows AR-502. It deletes nothing. |
| The hub does not recognize the device (AR-504) | Stops using the channel and shows the error. It deletes nothing. |
| Expired by inactivity (AR-507) | Stops using the channel and shows the error. It deletes nothing. |

"Stopping using the channel" does not end the connector: it keeps answering Hermes, with the error,
and keeps serving `join` and `leave` through the local socket.

## 7. Deadlines and limits

| Item | Value |
|---|---|
| Validity of the invite | 15 minutes |
| Deadline to type the verification code | 10 minutes from the first enrollment |
| Code attempts | 5 |
| Enrollments per request | 5 |
| Approved without confirming the first credential | 24 hours |
| Channel heartbeat | Every 30 seconds; 90 seconds with no answer end the connection |
| Check of whoever presents a credential | 5 seconds |
| Test of the old connection in a double connection | 3 seconds |
| Connection that has not become a session yet, apart from the waiting of the request | 90 seconds |
| Credential swap inside the session | Every 24 hours |
| Deadline of the "saved", in any swap | 60 seconds |
| Connections without confirming the swap that generate an alert | 3 in 24 hours |
| Notice queue per Hermes | 24 hours or 1,000 notices |
| Interval between notices of the same target | 10 seconds |
| Wake ceiling of the connector | 30 per minute and 300 per hour |
| Sweep of the hub: connected devices and queues | Every 2 seconds, through a set of database connections only for it |
| A frame that does not leave the hub | 30 seconds close the connection |
| Orderly stop of the hub | Up to 8 seconds for the calls in progress, and 2 more for the last `closing` |
| Check of the engine configuration | Every 5 minutes |
| Revocation: time until the channel drops | Under 5 seconds |
| Inactivity that requires reconnecting | 30 days |
| Send ceiling per Hermes and per number | 20 per minute |

The limits of the channel, per phase and per origin, are in [14-protocolo-do-canal.md](14-protocolo-do-canal.md),
section 4. The note ceilings, in [13-conversa-entre-hermes.md](13-conversa-entre-hermes.md), section 6. The
retention periods, in [15-mvp.md](15-mvp.md), section 2. To be defined: the default retention period of
the message archive.
