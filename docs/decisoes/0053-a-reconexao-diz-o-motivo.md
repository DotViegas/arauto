# 0053 — The reconnect says why

**Status:** proposal · **Date:** 19/11/2026. Asked and approved by the maintainer on 19/11/2026:
D1 option B, D2 option A (with the sentence for `docs/04`), D3 to D8 as recommended, D9 (the texts,
word for word) and D10 (this record). **Built in the hub and tested against a fake connector and a
fake Hermes; nothing was tried on a real Hermes.** The connector side (what `join` leaves running and
the build mark) is built and tested against a fake leader; the bundle number of the hub is built and
unit-tested; the image build was not run.

## Why it exists

A Hermes that was asked to connect again answered "I am already connected", read the deadline of an
invite in its own clock, or did not know why a new connection was being asked. The message did not
say. Now the message says why in one "Why:" line, asks to reconnect with its own first line, and
points to a "Reconnect" section of the page.

## What was decided (proposal)

### The reason: derived by the hub, once, under the lock

- The admin picks nothing. The hub derives the reason inside the write that issues the invite, from
  the rows of the Hermes it locked, **before** it cancels the invite issued earlier and closes the
  request in progress (that close would erase what the reason reads). It stores the reason on the
  invite row (migration 0024) and in the audit event `invite.issued` as fixed words.
- The previous invite is read in the same write: the newest of the Hermes by `(created_at, id)`.
- The newest closed device that tells something is the newest row in a final state that is not
  `replaced` and was not closed by a newer invite, and only when it is newer than the device in use
  (if there is one). It is also what the list uses (`newest_final`).

| Order | Reason (stored word) | When |
|---|---|---|
| 1 | `quarantined` | The device in use, or the request, is in quarantine. |
| 2 | `restored` | The device in use is suspended, or the newest closed one expired because the restored mode closed it. |
| 3 | `request_approved` | A request is approved and has not confirmed. |
| 4 | `request_pending` | A request waits for its code. |
| 5 | `approved_timeout` | No request; the newest closed one expired approved (AR-303). |
| 6 | `previous_request_expired` | No request; the newest closed one expired pending (AR-302). |
| 7 | `previous_request_denied` | No request; the newest closed one was denied, or revoked before it confirmed. |
| 8 | `expired_inactive` | No device, no request; the newest closed one had connected and expired for inactivity (AR-507). |
| 9 | `revoked` | The same, revoked by the admin (AR-502). |
| 10 | `left` | The same, closed at its own request. |
| 11 | `invite_unused` | The previous invite is newer than every device row, created none, and expired (or is issued and past its deadline). |
| 12 | `other` | None of the above: the admin asked for a new connection. |

The first that matches wins. The window of the actions previews the same reason with one read (no
lock, no write); the form carries what it showed, and a reason that moved in between is said on the
screen that shows the message. The message carries what the write saw.

### The first lines and the Why line

- **Three first lines.** The Hermes has a device in use: "...reconnect this Hermes... It has a
  connection to this hub; this replaces it." It has none but was connected before: "...It was
  connected before; this brings it back." Otherwise a first connection, with the text that was
  already approved.
- **The Why line** (second line) appears on every reconnect, and on a first connection only when an
  earlier attempt failed or an invite was not used (reasons 3 to 7 and 11). A first connection that
  has nothing to say keeps its six lines.
- **A clause about the connector** follows the reason when the connector of the device in use is
  older than what the hub serves ("changes from version X to version Y" only when both numbers are
  known and parse; otherwise "is older"), or is not one the hub serves. Only the numbers of a
  version that parses reach a text; no other value that a device declared ever does.
- **The read line** of a reconnect points to the "Reconnect" section of the page. **The invite
  line** of every sentence says for how many minutes the invite is valid from when it was issued,
  gives the UTC deadline, and says that the hub checks the time, not the Hermes.
- The texts are in [docs/02](../02-como-funciona.md), section 1 ("The reconnect sentence") and in
  the catalog of the panel (nine languages; the Portuguese was approved, the others were translated
  by the builder and **not read by a native speaker**).

### `join` and the running connector (D1, option B)

`join` stops ending the connector that is running, unless the folder format differs from the one
the old connector reads (then it ends it, as before, and says so). The Arauto tools keep working on
the old program during the reconnect; at the next restart of the gateway Hermes ends the old program
and starts the new file. Until then the panel shows "Restart pending", so nobody reconnects again
for nothing. This amends docs/02 section 7, [docs/11](../11-conector.md) and
[docs/14](../14-protocolo-do-canal.md). The old rule in docs/04 does not move: the device in use
goes on until the new one confirms its first credential.

### The build mark and the bundle (D2, option A)

- The connector has its own **build number**, not a published signed version, and declares
  `X.Y.Z+<64 hex of its SHA-256>` in `hello.connector`. The hub stores the program that runs
  (`connector_version`, `proto`) and, written once at the enrollment, the program that ran `join`
  (`enrolled_connector`, `enrolled_proto`). The format of `hello` does not change.
- The standing of the connector of the device in use is a hint computed from the protocol the hub
  saw and the declared mark and number: **restart pending**, **outdated**, **other build**,
  **current** or **not known**. A mark is compared with the digests of the files the hub serves; a
  mark that is not exactly 64 lowercase hexadecimal characters is no mark. The proposal's table, read
  literally, would call every unmarked connector at protocol 2 "restart pending"; the pairing used
  here is: the program that enrolled is a served one while what runs is not, or the enrollment was
  made on the current protocol and the connection still speaks an older one.
- **The bundle number** (the number of the connectors the image of the hub bundles):
  the image builds the connectors first and compiles their build number and digests into the hub
  (`ARAUTO_BUNDLE_CONNECTOR_VERSION`, `ARAUTO_BUNDLE_CONNECTOR_SHA256`; module `bundle`). At start
  the hub compares the files it serves with those digests. Only when all match is the bundle
  known; then messages carry the two numbers and an outdated connector has its explanation line.
  Otherwise (development build, another folder, any difference) nothing carries numbers.
- **The bump policy** (approved): the build number goes up by a patch in the commit that changes
  the bytes of the connector (code, `arauto-protocol`, `arauto-platform`, its dependencies in
  `Cargo.lock`), by a minor when `PROTOCOL_VERSION` changes, with `Cargo.lock` in the same commit;
  a hub-only change does not bump it. First bump: 0.1.0 to 0.1.1 (19/11/2026). The sentence of
  docs/04 that tells a build number apart from a published signed version is written
  (approved word for word).
- **Tested (fact, 19/11/2026):** the unit tests of the format of the bundle, of the comparison with
  the served files, of the numbers in the reason and of the panel line; the connector tests with
  0.1.1. A release build of the connector for Windows x86_64 (MSVC, no `tls`) was built twice
  from the same tree and path; the two digests differ (not reproducible there); see
  [referencia/build-fatos.md](../../referencia/build-fatos.md).
- **Not tested:** the image build (the development machine has no Docker); the Linux, macOS and
  cross builds; the hub started from an image with the bundle compiled in.

### The panel

- Labels of a line with no device in use that was connected before: **Revoked**, **Left**,
  **Expired** (it was "No device" before). Each offers **Reconnect** and nothing to revoke.
- A mark in the connector column: **Restart pending**, or the number of an outdated connector in
  the colour of waiting; the counters add "Outdated: {n}" when it is above zero.
- The window of the actions says: a connector that has not reached the hub since a time should
  have its gateway restarted first; an approval is confirmed when the connector runs, and may need
  one gateway restart; the preview of the reason.

### Times (D5, option A; approved 19/11/2026)

For a Hermes everything is UTC plus the duration from issue; the hub, not the Hermes, judges the expiry (the page has a section "The deadline"). For the admin, every absolute time of the panel is written by the hub as `<time datetime="...Z">YYYY-MM-DD HH:MM UTC</time>` and the script of the panel (`panel.js`, text only) rewrites it to the browser zone with a numeric offset (`2026-11-18 23:40 (UTC−04:00)`); without the script the labelled UTC stays. Lists stay relative. The invite window adds a plain line, "The Hermes reads the deadline in UTC", which is never localized. The admin command and the output of the hub stay UTC. Built; not tried in a real browser.

Pending: a zone chosen by the admin and stored (no setting, no migration yet).

## Amends

[docs/02](../02-como-funciona.md) sections 1 and 7; docs/04 (the sentence of D2); [0047](0047-notas-do-arauto-na-memoria.md) (the notes on a reconnect);
[docs/06](../06-modelo-de-dados.md); [docs/07](../07-telas.md); [docs/10](../10-maquinas-de-estado.md);
[docs/11](../11-conector.md); [docs/14](../14-protocolo-do-canal.md).

## What is not guaranteed

- Option B rests on reading, not on a trial: that an old leader follows a request created by a newer
  `join`, confirms it and moves the credential was not tried.
- Until the gateway restarts, the Hermes runs the older connector with the new device. A connector
  fix does not apply before that; the minimum version of the hub (AR-401) is what stops old code.
- The declared number and the mark are declared text. A copy can lie. The standing is a hint: it
  decides and blocks nothing. Only the protocol is seen by the hub.
- The reason is derived from state; the hub cannot tell a new machine, a lost folder, a key or
  address change, or a conversation with no terminal. Those get `other`.
- The preview can differ from the message when the state changes between the window and the issue;
  the screen says so, and the message carries the state the write saw.
- An LLM can still ignore the page, misread the deadline, save duplicate notes or answer "yes"
  without saving. The texts make the right path clear; they cannot force it.
- Builds of the same commit can differ (checked 19/11/2026 on Windows MSVC); "other build" can then
  appear for a device after a rebuild of the image. The image itself was not checked.
- Windows and macOS connectors were compiled and never run.
- The translations of the new texts into the eight other languages were not read by native speakers.
- Nothing here was tried on a real Hermes.
