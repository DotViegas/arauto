# 07 — Screens of the panel

**Status: proposal.** What each screen shows, what is done on it and how it looks empty or with an
error.

**Org chart, 14/11/2026** ([decision 0043](decisoes/0043-organograma-e-niveis.md), proposal): the hub has a fourth tab, T9, built and tried only in a desktop Chrome without a window (15/11/2026).

**Numbers in the panel, 15/11/2026** ([decision 0044](decisoes/0044-numeros-no-painel-sobre-o-motor.md), proposal): T4 has two buttons, "Connect WhatsApp (WAHA)" and "Connect WhatsApp (native · in test)" (the second only opens a window with a fixed text and does nothing), the actions on each line, and each action opens a window over the list. T5 is built as described below. It was written and covered by automatic tests against a fake engine only: **it was not tried in a real browser, nor from the panel against a real WAHA.** Where this note and the rest of the page differ about T4 and T5, this note wins. Disconnect and Remove ask for the alias typed and **not** the passkey again, because no step that asks for it exists (see the note on the session below and decision 0040, pending decision 2).

**In the hub, on 13/11/2026** ([decision 0040](decisoes/0040-painel-dentro-do-hub.md), proposal):
there are T1, T4 and T6, read-only, the alert banner, and T2 with T3, in a single guide, in a window
over the list: name, block, code and done. In T1 each line has the buttons to connect, continue or
reconnect, and the one to revoke, which asks first; delete is still only in the
command (15/11/2026, [decision 0045](decisoes/0045-a-marca-da-ligacao-basta-para-acordar.md): the switch "woken by WhatsApp" is set in the access editor too). In T6 each cell opens the access editor, in a window; choosing the chats of the scope
"only chosen chats" is still only in the command. The other screens are still
design. T0 exists in part (see below): one gets in with a passkey, whose first registration is
opened by an installation code. The panel session lasts 12 hours, or 30 minutes without use, and
it is the session that authorizes the actions: asking for the passkey again on every action is
not built.

## Summary

```
Menu:  Hermes · WhatsApps · Accesses · Org chart · Audit · Settings

T0 First access         create the first admin and point to the engine
T1 Hermes               the list, with the state of each one
T2 Connect Hermes       the connection sentence and the wait
T3 Confirm connection   type the verification code
T4 WhatsApps            the list of numbers
T5 Pair WhatsApp        pairing code or QR
T6 Accesses             the Hermes x number matrix
T7 Audit                what happened, with no message content
T8 Admins               who has a login in the panel (in Settings)
T9 Org chart            levels, places and links of the Hermes, on a map
```

## Rules that apply to all screens

| Rule | Reason |
|---|---|
| **The panel is the source of truth, not what the LLM says.** Each step lights up by a fact the hub observed. | A Hermes may say "done" without having done it. |
| **Every text coming from a device or from WhatsApp is treated as hostile**: validated on the way in, escaped on the way out. | Machine name, contact name and group subject are written by third parties. |
| **The panel states the limit of the protection.** Each Hermes shows "Basic protection: the LLM reaches the credential file" or "Shielded protection". | In quick mode the LLM reaches the credential file. |
| **Every action that changes something asks for the passkey on the spot.** With the panel session, alone, one only sees. | A session stolen from the browser cannot be enough to act. |
| **Invite, QR and pairing code appear once** and never go into a log. | They are short-lived secrets. |
| **The panel lists what goes through the hub.** Other linked devices on the same number only appear on the phone. | The hub does not see what does not go through it. |

Planned, with the means still to be defined ([decisoes/0025](decisoes/0025-aviso-por-fora-do-painel.md)):
every approval and every grant notifying the admin outside the panel.

Above the menu there is an alert banner, visible on any screen: another device tried to use an
invite, identity copied before the approval, Hermes in quarantine, connection taken by another,
closing requested by the device, reuse after the closing, three swaps without confirmation in 24
hours, new network or country, request waiting for the code, engine unavailable, engine
configuration corrected by the hub, number disconnected, hub restored from a backup.

The panel session lasts 12 hours, or 30 minutes without use. The waiting screen updates by itself,
by periodic query to the hub.

The lists (Hermes, WhatsApps, Accesses) and the alerts above the menu refresh by themselves every 5
seconds, by asking the same page again and swapping only their content; there is no permanent
connection. That request does not count as use of the panel session, so a tab nobody touches still
ends at the idle time and then shows the entry page. It needs the script of the panel: without it
the page is as before. **Not tried in a real browser yet** (14/11/2026). A list is not swapped
while one of its controls has the focus; a browser that does not give the focus to a button on a
click could lose a click that lands in the instant of a swap.

## T0. First access

- **Shows:** three steps. (1) The installation code, which the hub writes in the output of its
  start until the first passkey. (2) The registration of the passkey of the first admin, at the
  address of the panel. (3) A window that checks the installation: the addresses of the panel and
  of the connectors, the address the hub sees, the password of the database, the connector, the
  data folder and whether an engine is set. The engine setup (address, API key, a "Test" button)
  is still a proposal (see below).
- **Rule:** without the installation code nobody creates the first admin. It works once. The one
  the hub writes in its log at the start has no deadline and is written again every 10 minutes
  while it is unspent; each start writes a new one, and the latest is the one that works. Typing
  it does not spend it: creating the passkey does. Once the first passkey is kept, the start no
  longer writes a code; a new one only comes out by a command on the server, valid for 10
  minutes, for when the only admin loses the passkey
  ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)).
- **Errors:** wrong installation code (one sentence for every refusal, which does not say whether
  the code was wrong, spent, replaced or past its time); engine does not answer, key refused or
  version not tested. The engine setup may be left for later.
- **In the hub, on 13/11/2026** ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)).
  *Built and tested by the automatic tests (13/11/2026):* the lifecycle of the installation code
  described in the rule above; the block the hub writes in its own output, with the code, at the
  start and every 10 minutes until the first passkey (its heading carries the date, `YYYY-MM-DD
  HH:MM:SS UTC`, and no line of fixed text is over 80 columns); the entry by state (step 1 with
  no passkey form before the first access; the page of who is back after it, with "Lost your
  passkey?" under the passkey, open when a code was refused); step 2 (the registration, which
  shows the address of `ARAUTO_PANEL_URL`); step 3 as a check window (`/installation`, also by
  `arauto-hub admin check`, which prints the same rows from the same function) with the addresses
  of the panel and of the connectors, the address the hub sees, the password of the database, the
  connector, the data folder and the engine; the same refusal page for every refused code; the
  ceilings of attempts per address and for everyone, in separate buckets for codes and for
  challenges. In the entry page, the command to read the log with Docker is
  `docker compose --env-file arauto.env logs hub`. The automatic tests ran against a test hub and
  a fake authenticator: no real browser, Easypanel or proxy has run them.
  *Still a proposal:* the engine step (address, key and a "Test" button): the check window only
  says whether an engine is set, and never shows its address or key.
  *Pending decision (the maintainer's approval of the text):* three English lines that only
  `arauto-hub admin check` prints, and that are outside the approved texts: "Not set in this
  environment.", "Only the running hub sees it: open the panel and the check window." and
  "Replaced when the first passkey is registered.". The window shows no sentence for the state
  "later" of the database password.
  The step 1 sentence that says which block to take is "Find the ARAUTO FIRST ACCESS block: the
  newest one is the last, at the bottom of the log. Copy its code." (approved on 13/11/2026): it
  asks for no search, since the Easypanel log viewer has none.

## T1. Hermes

- **Shows:** counters ("5 connected, 1 waiting, 0 in alert") and one line per Hermes: name,
  state ([10](10-maquinas-de-estado.md), section 3), numbers with access, whether WhatsApp may wake it ("Woken by WhatsApp"),
  connector version, last seen.
- **Hermes card:** the limit of the protection; observed address, current and previous; what the
  device declared, marked as not verified; date of the connection; notices in the queue; latest
  events. Fixed text: "The tools appear on all of this Hermes's conversation platforms. Whoever can
  give it orders can use the granted numbers."
- **Actions:** Connect Hermes; Accesses; Reconnect; Rename; Revoke (asks for the name typed);
  Delete (only with no invite issued and no device pending, approved, active, suspended or in
  quarantine).
- **Empty:** "No Hermes connected. You send a message to Hermes's command channel and type here
  the verification code it answers."
- **Quarantine:** "The hub no longer trusts the credential of {name}. Reason: a credential already
  swapped was presented | two connections at the same time | unreadable credential | the last use
  was not recognized on reconnecting. The access is stopped." Ways out: Reconnect or Revoke. There
  is no "release".
- **Connection taken:** "The connection of {name} was taken by another, coming from {address}." It
  appears whenever the hub takes for dead a session that was serving and another comes in its place.
- **Closed by the device itself:** "{name} asked for its own closing, from {address}. Last
  use: {date and time}, from {address}."
- **Reuse after the closing:** for a device that was revoked, replaced or expired by inactivity
  and that presents an old credential: "A copy of {name} was used before the closing."
- **Hub restored:** a banner on all screens: "The hub came back from a backup of {date}. What was
  approved, revoked, reconnected or changed in the accesses after that was lost. Reconnect each
  Hermes and check the accesses and the list of admins." The Hermes with an active device appear as
  suspended, each one with "Reconnect" and "Revoke". There is no "reactivate". The devices that
  presented a valid seal the database does not explain appear in a separate list, with time, address
  and what they declared.

## T2. Connect Hermes

```
Connect Hermes: Ana                   The invite is valid for 14:32 more · single use
┌──────────────────────────────────────────────────────────────────┐
│ (the connection sentence, read-only, in the language of the panel) │
└──────────────────────────────────────────────────────────────────┘
Send it as a new message, not as a reply to one of its notices. If it asks
you to confirm, answer in your own words.
Click the message to copy it.    Cancel invite
> Show the command instead      (the old block; send one or the other, not both)

1. Send it in the command channel of Hermes "Ana". Nowhere else.
2. It will answer a 6-digit verification code. Come back here.

[x] Invite issued                 14:02:11
[ ] A device used the invite      waiting...
[ ] Connector up
[ ] Code confirmed by you
[ ] First credential confirmed
```

- **The steps:** "Connector up" lights up on the first channel connection of the device, waiting or
  already in the swap. It and the one for the code light up in any order.
- **Name:** unique, 3 to 32 characters, only letters, numbers, space and hyphen. A short name with no
  symbols reduces what it manages to carry into the sentence.
- **The sentence only exists on this screen and in this tab.** The hub does not keep the invite.
  Reloading shows "The invite is valid for mm:ss more, but the message cannot be shown again". The
  instruction page it points to keeps serving until the request is decided or the invite expires
  ([14](14-protocolo-do-canal.md), section 1).
- **Status: proposal.** The sentence and the page were not tried on a real Hermes yet.
- **There is no "send by e-mail" or "send by WhatsApp".** A sentence or block that reached Hermes through
  a number of the hub would arrive as data, and must be refused.
- **If Hermes asks for approval to run the command,** the one who approves is you, in the command
  channel.
- **Timed help:** 90 seconds without an enrollment open "Nothing has arrived yet", with the three
  causes: Hermes is waiting for your approval; Hermes does not reach the hub; the message was sent
  somewhere else.

## T3. Confirm connection

```
A device used the invite of "Ana"                     confirm within 09:41

Seen by the hub       address 203.0.113.7 · 14:03:02
Said by the device    machine hermes-ana · connector 1.0.0 · Hermes 0.22.5
                      (the device may lie here; what decides is the code)

Type the verification code Hermes answered in the command channel:
        [ _ _ _ ] - [ _ _ _ ]

[ Approve ]      [ Deny ]      I received no code at all
```

- **The panel does not display the verification code.**
- **Approve** only with the right code, and it asks for the passkey.
- **"I received no code at all":** "If Hermes did not answer a code, do not approve. If it answered
  AR-102, another device used the invite: deny and generate another."
- **States:** wrong code ("attempt 2 of 5"); 10-minute deadline run out; another device
  tried the same invite (alert, and the current request stays valid); Hermes ran the command
  again ("the last code it answered is the one that counts").
- **On reconnecting,** before the code: "Last use of the current device: {date and time}, from
  address {address}. Do you recognize it?" If Hermes was off and there was use, someone used a copy.
  "I do not recognize it" raises the copy alert, puts the old device in quarantine if it was active
  (suspended or in quarantine, it stays as it is), shows its calls since then and lets the
  approval go on.
- **After approving:** "Ana was approved. As soon as the connector confirms, it appears as
  connected." On a first connection, it is followed by "It has no access to any number yet", with
  the button "Give access". On a reconnect, it is followed by "The accesses Ana already had are
  valid again for this device", with the button "See accesses".

## T4. WhatsApps

- **Shows:** one line per number: alias, paired number, state, whether it is "command", Hermes with
  access, time of the last message received (no content), and an Actions column (built on
  15/11/2026; the other columns are as before).
- **Buttons of the tab:** "Connect WhatsApp (WAHA)" opens T5. "Connect WhatsApp (native · in test)"
  opens a window that says, in fixed text, that an engine built into Arauto is only being studied
  and is not part of this version, and that for now numbers connect through WAHA. It has no form
  and does nothing. A built-in engine is **not decided** (decision 0044).
- **Actions on each line, by state** (built; a button that the state does not allow is not shown):
  Pair or Pair again (unpaired, disconnected, or pairing with no run in the hub); Continue pairing;
  Restart the engine session (failed); Disconnect from WhatsApp (connected, reconnecting); Decide
  (number swapped); Remove (any state). "Mark as command" is offered **only when the number is
  added**; changing it afterwards is not built, because no function does it.
- **Disconnect and Remove ask for the alias typed, and not the passkey again.** The passkey on every
  action (decision 0019) is not built (decision 0040, pending decision 2). The alias typed, the
  `POST` with the panel's `Origin` and the full panel session are the strictest check that needs no
  new mechanism. Remove also needs one choice, with none preselected: keep the history in the
  engine, or erase it there. The message archive is not built.
- **First number:** acceptance recorded in the audit: "The engine is an unofficial WhatsApp
  client. WhatsApp may block the number. Use numbers you can afford to lose." In the panel it is
  asked only for the first number the organization ever had (removed ones count) and is recorded
  as `risk_accepted` on `number.added`. The command does not ask for it.
- **Errors:** the window says plainly when no engine is set (and points to `arauto-hub admin
  engine ...`: the engine key is a long-lived secret and is never typed on a screen), when the
  engine has no key, does not answer, refuses the key or answers something unexpected. The
  sentences below are older design text: "Engine unavailable: the hub has not talked to the engine since HH:MM. The Hermes stay
  connected and read what is already in the message archive; they do not send or receive new
  messages."; "Disconnected: the linked device was removed on the phone"; "This number sends, but
  the hub is not receiving its messages",
  with the Reconfigure button.

## T5. Pair WhatsApp

Built in the hub on 15/11/2026, in windows over the list (decision 0044, proposal).

- **Step 1:** alias and number with country code, and the box "command". For the first number the
  organization ever had, also the acceptance sentence of T4, which must be ticked.
- **Step 2:** the choice of QR or pairing code.
- **Step 3:** the QR, drawn by the hub as an image in the page, which renews itself in the window,
  or the pairing code with the path on the phone and the time left (the hub's deadline, 3 minutes;
  the validity on WhatsApp's side is not known). Then "Paired: +00 00 00000-0000." If the paired
  number is another one: "The phone that paired belongs to a number different from the one given.
  Keep this one | Undo." Undo needs a checkbox that confirms erasing the engine session's history.
  While the admin does not decide, the state is "number swapped" and the accesses stay stopped.
- **Cancel the pairing** stops the engine session and puts the number back to unpaired. **Closing
  the window does not cancel**: the pairing runs until its deadline, and the line offers "Continue
  pairing" to the session that started it. Another panel session sees that a pairing is running
  but never its QR or code.
- **Ceilings:** one pairing per number and three at a time per organization.
- **The QR and the code** are short-lived secrets: shown only in the panel, to the full panel
  session that started the pairing, in answers that are not cached, never logged, never audited and
  never written to the database. The hub drops them when the pairing ends.
- **Without script** the window reloads itself every 3 seconds and each reload carries the current
  QR; the countdown moves only on a reload. With script, each QR value travels once per window.
- **Not guaranteed:** see decision 0044. In short, nothing was tried in a real browser or against a
  real WAHA; after a hub restart a running pairing is lost and the row stays "pairing" for up to
  about 8 minutes; a stolen panel session can pair, keep, disconnect and remove numbers.

## T6. Accesses

The Hermes are in the rows; the numbers, in the columns.

```
Accesses          [ by Hermes | by number ]            filter: ________

                  Counter           Purchasing        Front desk
                  +00 ..0001        +00 ..0002        +00 ..0003
Hermes Ana        R S - W  all      - - - -           R S - W  all
Hermes Bento      R - - -  3 chats  R S M W  all      - - - -
Hermes Old        in quarantine (accesses preserved)

R read · S send · M modify · W be woken
(!) Counter: 2 Hermes are woken in the same chats
```

- **Cell:** scope (all | only chosen chats), the four verbs, "may start
  chats", chat lines. The screen writes the resulting sentence: "Hermes Bento may read in 3
  chats of the number Counter. It does not send, does not modify and is not woken."
- **Save** applies on the next call, without restarting anything.
- The editor also holds the switch "Woken by WhatsApp" of the Hermes, for every number; notes do not depend on it ([decision 0045](decisoes/0045-a-marca-da-ligacao-basta-para-acordar.md)).
- **Warnings that do not block:** two Hermes woken or sending in the same chats; a restrictive chat
  line under the scope "all"; **send** granted on a number marked as "command".
- **Empty:** "Connect a Hermes first"; "Add a WhatsApp first"; "Nobody has access to
  anything. Click a cell to grant."

## T7. Audit

- **Shows:** timeline, from the most recent to the oldest: when, who (admin, Hermes,
  system), what, where (Hermes, number, chat), result, AR code, address.
- **Events:** invite issued, used, expired, cancelled and refused (AR-101); attempt by another
  device; verification code right and wrong; approval and denial; connection opened and closed;
  credential swapped;
  connection that ended without confirming the swap; connection taken by another; quarantine;
  revocation; closing requested by the device; reuse after the closing; use not recognized;
  two waiting connections with the same identity; new network or country; device replaced; access
  changed, with before and after; login; admin added and removed; passkey created and removed;
  number added, paired,
  removed; tool call; denial for lack of a verb; notice confirmed, expired and
  discarded; engine configuration corrected by the hub; hub restored from a backup; device
  suspended; unknown device with a valid seal.
- **Never contains:** message content, invite, credential, QR, pairing code. The end of a pairing (expired, failed or cancelled) is not audited, as today in the command; the start of a pairing is, and so is the add of a number.
- **Actions:** filter, open the detail, export (asks for the passkey). There is no edit or delete.

## T8. Admins

It is in Settings.

- **Shows:** the admins and the passkeys of each one, with the date of creation and that of the last
  use.
- **Actions:** add admin (the new one registers their own passkey), remove admin, remove passkey.
  All ask for the passkey of whoever does it. The last admin cannot be removed.
- **After the hub comes back from a backup,** the banner tells to check this list: an admin removed
  after the backup appears here again.

## T9. Org chart

**Proposal, built in the hub on 14/11/2026, tried only in a desktop Chrome without a window (15/11/2026).** Decision [0043](decisoes/0043-organograma-e-niveis.md). The menu has it after Accesses.

- **Shows:** a map with one band per level, the top one first, and a card per Hermes (state dot, name, the numbers it has and whether it answers or only reads, "not placed yet" when it has no place). Arrows mark the links. Their kind comes from the levels: from a higher level to a lower one, instruction; lower to higher, request; same level, note. The kind is only shown here; it gives no permission. A list view shows the same data (levels, Hermes, links) and is the way for the keyboard: everything the map does is there. Only Hermes are on the chart.
- **Actions (all on a draft):** add, rename, reorder and remove (when empty) levels, from 1 to 12; drag a Hermes to another level or place; draw a link by dragging from the card's handle to another card; click an arrow for its kind, the wake mark (the window says the mark is enough to wake by a note), the way back and removal; write a role text of up to 280 characters per Hermes, for the admin's own reading; undo.
- **Publish:** shows a summary of the changes, with the warning that links are security rules and that a link crosses numbers, and writes everything in one go. Nothing is written before it. The links, with their wake mark, go through the same code as the command `admin link add|remove`.
- **Stale:** if the chart changed meanwhile (another admin, a command, a Hermes created or removed), nothing is published, the page shows the fresh state and keeps the draft to apply again.
- **Empty:** "No Hermes yet. Connect the first one in the Hermes tab." With no level, the draft starts with one level holding every Hermes.
- **Without script:** three read-only lists (levels, Hermes, links) and a sentence saying that editing needs the script.
- **Live refresh:** the tab refreshes the states like the others, but pauses while there is a draft.
- **Narrow screens:** the view starts as a list; the map scrolls sideways.
- **Does not do:** edit accesses to numbers (the card shows them and points to Accesses); edit the WhatsApp switch (the drawer shows it in its numbers section; it is set in the access editor and by the command); put people on the chart; put a Hermes in two levels; tell any Hermes about levels, roles or kinds (stage B, pending). The command twin is `arauto-hub admin org show` and `org publish`.
- **Not guaranteed:** the draft is lost if the panel session ends before Publish; Publish does not ask for the passkey again (the note on top of this file covers it).
