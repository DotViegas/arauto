# 07 — Screens of the panel

**Status: proposal.** What each screen shows, what is done on it and how it looks empty or with an
error.

**Org chart, 14/11/2026** ([decision 0043](decisoes/0043-organograma-e-niveis.md), proposal): the hub has a fourth tab, T9, built and tried only in a desktop Chrome without a window (15/11/2026).

**Numbers in the panel, 15/11/2026** ([decision 0044](decisoes/0044-numeros-no-painel-sobre-o-motor.md), proposal): T4 has two buttons, "Connect WhatsApp (WAHA)" and "Connect WhatsApp (native · in test)" (the second only opens a window with a fixed text and does nothing), the actions on each line, and each action opens a window over the list. T5 is built as described below. It was written and covered by automatic tests against a fake engine only: **it was not tried in a real browser, nor from the panel against a real WAHA.** Where this note and the rest of the page differ about T4 and T5, this note wins. Disconnect and Remove ask for the alias typed and **not** the passkey again, because no step that asks for it exists (see the note on the session below and decision 0040, pending decision 2).

**In the hub, on 13/11/2026** ([decision 0040](decisoes/0040-painel-dentro-do-hub.md), proposal):
there are T1, T4 and T6, read-only, the alert banner, and T2 with T3, in a single guide, in a window
over the list: name, block, code and done. In T1 each line has the buttons to connect, continue or
reconnect, and the one to revoke, which asks first; delete is still only in the
command (15/11/2026, [decision 0045](decisoes/0045-a-marca-da-ligacao-basta-para-acordar.md) added the switch "woken by WhatsApp" to the access editor; 16/11/2026, [decision 0049](decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md) removed it). In T6 each cell opens the access editor, in a window; choosing the chats of the scope
"only chosen chats" is still only in the command. The other screens are still
design. T0 exists in part (see below): one gets in with a passkey, whose first registration is
opened by an installation code. **Times.** Every absolute time the panel shows is written by the hub as a `<time datetime="...Z">` element, in UTC and labelled (`2026-11-14 12:34 UTC`). With the script of the panel, the text is rewritten to the zone of the browser with its numeric offset for that instant (`2026-11-14 08:34 (UTC−04:00)`), and the UTC text stays as the title of the element; without the script (or without `Intl`) the labelled UTC stays. Lists show relative times ("3 h ago"). What a Hermes reads stays UTC (the connection message and the page of the invite), and the admin command and the output of the hub stay UTC, labelled. The window of an invite gives its deadline in both forms: the `<time>` element, and a plain line saying that the Hermes reads the deadline in UTC. The script writes text only (`textContent` and `title`). Built; **not tried in a real browser yet** (19/11/2026).

The panel session lasts 12 hours, or 30 minutes without use, and
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
  state ([10](10-maquinas-de-estado.md), section 3), numbers with access,
  connector version, last seen. (16/11/2026, [decision 0049](decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md): the column "Woken by WhatsApp" is gone; whether WhatsApp may wake a Hermes is read from the verb "be woken" of its accesses.)
- **Hermes card:** the limit of the protection; observed address, current and previous; what the
  device declared, marked as not verified; date of the connection; notices in the queue; latest
  events. Fixed text: "The tools appear on all of this Hermes's conversation platforms. Whoever can
  give it orders can use the granted numbers."
- **Who gives it orders (17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md), part B, proposal: built and tested against a fake engine, not tried in a browser):** every line has the action "Who gives it orders", with its state in the muted style: "not recorded", "its own chat", "1 telephone" or "{n} telephones", or "outside WhatsApp" (18/11/2026, R3, proposal, not muted: it counts as recorded). It opens a window over the list, "Who gives {hermes} orders on WhatsApp", at `/hermes/whatsapp?name=`. (18/11/2026, option B of the window, proposal: built and tested against a fake engine, **not tried in a browser**.) The numbers of the hub are offered as choices, each by its alias with its telephone. "Its WhatsApp" is a radio: it has no WhatsApp, one of the numbers (a list box above 8 numbers; a number where another Hermes already takes orders cannot be chosen), or another telephone, typed. "Orders come from" is its own chat, "These telephones" or (18/11/2026, R3, proposal: built and tested against a fake engine, not tried in a browser) a third option, "Outside WhatsApp: another conversation platform, like Telegram", with the hint "Arauto then refuses nothing for {hermes}. The hub cannot check it: if {hermes} also takes orders on a WhatsApp, record that WhatsApp instead." Outside WhatsApp together with a telephone, a checked number or a typed line is refused: "Orders from outside WhatsApp take no telephone: choose It has no WhatsApp, and leave These telephones and Other telephones empty. Nothing was saved." A save to it asks nothing unless it unprotects chats (then the window that asks names them), and says "Saved. {hermes} takes its orders outside WhatsApp: Arauto refuses nothing for it." As for the list option: a box per number of the hub (the box of its own WhatsApp, marked "its WhatsApp", comes disabled unless it came back checked after a refusal, 18/11/2026), and "Other telephones", one per line, for telephones that are not numbers of Arauto, in a box tall enough for every telephone recorded (up to 12 lines); at most 16 telephones in all, counted as the command counts them (both forms of a Brazilian mobile once). The form sends aliases; the hub reads their telephones and saves telephones. A typed telephone that does not start with "+" stops the save once: the window comes back, with nothing saved, saying how the hub read each line (the telephone, the number of Arauto it is or that it is not one, a repeated line, a line that is not a telephone, "n of 16"); saving again with the same lines keeps them as read. The error is at the top of the window, tied to its field. Another Hermes taking orders on the chosen telephone is said before the window that asks. Its own chat together with a list is refused, as in the command. The window says that Arauto refuses every Hermes, the Hermes included, writing in that chat, and carries one fixed sentence about the risk **whatever is recorded**. Saving calls the same function as `arauto-hub admin hermes whatsapp` (same rules, same audit, same refusals). A save that leaves no chat protected (a first save with everything empty included) or that unprotects a chat first shows a window over the first one, "A Hermes could give {hermes} orders", naming the chats that stop being protected (alias first), with "Remove the protection" or "Save without protection" and "Go back"; its close button goes back too; "Go back" returns with every choice and what was typed, and **nothing is saved before the answer**. A save that protects more asks nothing. The window that asks for a telephone the connector reported (part E) is **not built**. Not built either: a Hermes removal in the panel (the panel only revokes; the line "Arauto stops refusing writes in the chat where {hermes} takes orders." is printed by `arauto-hub admin hermes remove` when the Hermes had a record).
- **One "Actions" button per line (19/11/2026, option 2 of the row actions: built, not tested, not tried in a browser):** the lists of Hermes (T1) and of WhatsApps (T4) show, in each line, one button, "Actions" (three dots and the word), a plain link to a window over the list: `GET /hermes/actions?name=` and `GET /whatsapps/actions?alias=`. The window has the name or alias as title, the state (and the numbers of a Hermes, or the telephone of a number) and the list of every action the line offered before, in its current state, each with an icon, its label, the value it has now (only "Who gives it orders") and one line that says what it does. The actions that end something (Revoke, Disconnect, Remove) come last, in a group of their own, and lead to the same questions as before; an action that posted a form still posts it. The window adds no question and changes nothing an action does; opening it writes nothing. "Who gives it orders" is a column of the Hermes list, its value a link to the window of T1; the tags of a number sit on a second line under its alias. An unknown name or alias goes back to the list.
- **The reason of an invite and the standing of the connector (19/11/2026, [decision 0053](decisoes/0053-a-reconexao-diz-o-motivo.md), proposal: built, tested without a real Hermes, not tried in a browser):** the window of the actions of a line previews the reason the message would give ("If you issue it now, the message tells the Hermes: Why: {reason}", only when the message has a Why line; the form carries the reason it showed in a hidden field, `reason_seen`). A line that was connected before and has no device offers "Reconnect" in that window, with nothing to revoke. A line whose label is "Disconnected" says that the connector has not reached the hub since a time, that a restart of its gateway comes first, and that a reconnect is only for a move to another machine or a lost .arauto folder. A line approved and waiting for the connector says that the connector confirms the approval when it runs, that its gateway must restart once if the Hermes said its program was replaced or a key was made, and that another invite cancels the approval (the same sentence is on the screen after the approval). On the screen that shows the message, when the state changed between the window and the write, a line says: "The state changed after you opened this window. The message gives the reason as it is now: {reason}". In the list, a connector that is older than what the hub serves shows its number in the colour of waiting and, when the hub knows its bundle number, a line says so ("{name} runs an older connector ({old}); this hub serves {new}. Reconnect it to update. Its Arauto tools keep working until its gateway restarts, and the new connector runs from that restart."); that line shows only with a known bundle (a hub built by the image whose served files match the digests compiled into it; proposal, the image was not built), and a development build has none. A connector whose gateway has not restarted shows "Restart pending", with a line that says no reconnect is needed; a connector that is not one the hub serves has a line that says so. The line of counters adds "Outdated: {n}" when it is above zero. The labels are "Revoked", "Left", "Expired" and "Restart pending". The texts of the reason are in decision 0053.
- **Actions:** Connect Hermes; Accesses; Reconnect; Rename; Revoke (asks for the name typed;
  18/11/2026: built and tested against a fake engine, not tried in a browser); Delete (only with no
  invite issued and no device pending, approved, active, suspended or in quarantine). After revoke,
  deny or a save of who gives it orders, the list says what the Hermes is now, in the line between
  the heading and the table (same date and status).
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
- *Built (18/11/2026, tested against a fake engine, not tried in a browser):* the waiting screen
  says that the message cannot be shown again and offers "Issue another invite", which cancels the
  one before. A form that issues an invite carries what it replaces; a resent or stale one (browser
  Back, reload, an old tab) issues nothing and leads to the waiting screen.
- **There is no "send by e-mail" or "send by WhatsApp".** A sentence or block that reached Hermes through
  a number of the hub would arrive as data, and must be refused.
- **If Hermes asks for approval to run the command,** the one who approves is you, in the command
  channel.
- **Timed help:** 90 seconds without an enrollment open "Nothing has arrived yet", with the three
  causes: Hermes is waiting for your approval; Hermes does not reach the hub; the message was sent
  somewhere else (built, same date and status; shown from 90 seconds after the invite was issued,
  also without script).

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

- **Shows:** one line per number: alias, paired number, state, Hermes with
  access, time of the last message received (no content), and an Actions column (built on
  15/11/2026; the other columns are as before).
- **Buttons of the tab:** "Connect WhatsApp (WAHA)" opens T5. "Connect WhatsApp (native · in test)"
  opens a window that says, in fixed text, that an engine built into Arauto is only being studied
  and is not part of this version, and that for now numbers connect through WAHA. It has no form
  and does nothing. A built-in engine is **not decided** (decision 0044).
- **Actions on each line, by state** (built; a button that the state does not allow is not shown):
  Pair or Pair again (unpaired, disconnected, or pairing with no run in the hub); Continue pairing;
  Restart the engine session (failed); Disconnect from WhatsApp (connected, reconnecting); Decide
  (number swapped); Remove (any state). (17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md): the "command" mark of a number, which the
  Add window offered and could not change afterwards, is gone. Who gives a Hermes orders is recorded
  per Hermes, by the command `arauto-hub admin hermes whatsapp` and, since part B (built and tested against a fake engine, not tried in a browser), by the window of T1. Each number that takes part in a record carries a tag, linked to the window of that Hermes: "gives orders to {hermes}" (the number is a telephone that gives it orders) or "{hermes} takes orders here" (the Hermes's own WhatsApp runs on it). Each line also has the action "Orders" (a number that is not removed), at any time after the number is in the list: a short form asks which Hermes (each with what the number is for it now) and whether the number gives it orders or is its WhatsApp, with nothing chosen beforehand, and opens that Hermes's window with the number already chosen (18/11/2026); the tags and this way in return to this tab after a save or a cancel. The address names the alias, never a telephone. That is only a fill: nothing is saved until the admin saves, and the window that asks first applies as in T1.)
- **The alert band (19/11/2026: built, not tested, not tried in a browser):** each alert of the band above the menu has a "Dismiss" button, a form that posts the id of the alert (never in an address) to `POST /alerts/dismiss`. It calls the function behind `arauto-hub admin alerts --ack`, which only marks the alert as seen; neither the command nor the button writes an audit event or treats a kind apart, so every listed alert has the button. The answer goes back to the list the admin was on (only `/`, `/whatsapps` or `/accesses`), else to `/`.
- **What a write says (18/11/2026: built and tested against a fake engine, not tried in a browser):** every form of a number ends on the list address, `/whatsapps?alias=` (with `&said=` and one fixed word when WAHA did not answer or the hub refused), so a reload or the refresh of the list never sends the form again. The line between the heading and the table takes the tone of what it says: the state of the number in its own color, a WAHA that did not answer in the amber band, a refusal in red, the command in it as code. A removed number is said removed.
- **Disconnect and Remove ask for the alias typed, and not the passkey again.** The passkey on every
  action (decision 0019) is not built (decision 0040, pending decision 2). The alias typed, the
  `POST` with the panel's `Origin` and the full panel session are the strictest check that needs no
  new mechanism. Remove also needs one choice, with none preselected: keep the history in the
  engine, or erase it there. The message archive is not built. The alias typed is shown as it is
  spelled; the label around it keeps its style. A refused Remove keeps the choice that was made;
  the alias is typed again only when it did not match (18/11/2026: built and tested against a fake
  engine, not tried in a browser).
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

- **Step 1:** alias and number with country code (the box "command" was removed on 17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md)). For the first number the
  organization ever had, also the acceptance sentence of T4, which must be ticked.
- **Step 2:** the choice of QR or pairing code. It says the number is already in the list; its way
  out is "Pair later", which leaves the number unpaired and says so on the list (18/11/2026: built
  and tested against a fake engine, not tried in a browser).
- **Step 3:** the QR, drawn by the hub as an image in the page, which renews itself in the window,
  or the pairing code with the path on the phone and the time left (the hub's deadline, 3 minutes;
  the validity on WhatsApp's side is not known). Then "Paired: +00 00 00000-0000." If the paired
  number is another one: "The phone that paired belongs to a number different from the one given.",
  then the telephone given and the telephone that paired, side by side. "Keep this one | Undo…";
  neither is the main button. Undo needs a checkbox that confirms erasing the engine session's
  history. (18/11/2026: built and tested against a fake engine, not tried in a browser.)
  While the admin does not decide, the state is "number swapped" and the accesses stay stopped.
  A pairing that ends without pairing keeps the path on "Phone"; only a paired number reaches
  "Done" (18/11/2026: built and tested against a fake engine, not tried in a browser).
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
- The editor holds no switch of the Hermes: the checkbox "Woken by WhatsApp" added by [decision 0045](decisoes/0045-a-marca-da-ligacao-basta-para-acordar.md) is removed (16/11/2026, [decision 0049](decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md)). "Be woken" is the only checkbox for waking by WhatsApp; its hint says: "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule." Turning it off removes the verb from those chat lines in the same write; turning it on does not touch them. Notes do not depend on it.
- **Warnings that do not block:** two Hermes woken or sending in the same chats; a restrictive chat
  line under the scope "all". (The warning for **send** on a number marked "command" went with the mark, 17/11/2026, [decision 0051](decisoes/0051-quem-da-ordens-a-um-hermes-no-whatsapp.md).)
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

- **Shows:** a map with one band per level, the top one first, and a card per Hermes (state dot, name, the numbers it has and whether it answers or only reads, "not placed yet" when it has no place). Arrows mark the links. Their kind comes from the levels: from a higher level to a lower one, instruction; lower to higher, request; same level, note. The kind is only shown here; it gives no permission. A list view shows the same data (levels, Hermes, links) and is the way for the keyboard: it does what the map does, except the follow box (it has none and only keeps the mark a link already has). Only Hermes are on the chart.
- **Actions (all on a draft):** add, rename, reorder and remove (when empty) levels, from 1 to 12; drag a Hermes to another level or place; draw a link by dragging from the card's handle to another card; click an arrow for its kind, the wake mark (the window says the mark is enough to wake by a note), the way back and removal; on every instruction arrow, a checkbox for the follow mark (proposal, [decision 0050](decisoes/0050-etapa-b-marca-de-seguir.md), built, not tried in a browser: it is disabled until the wake mark is ticked, with a hint that it is a sentence to an AI and not a control); write a role text of up to 280 characters per Hermes, for the admin's own reading; undo.
- **Publish:** shows a summary of the changes, with the warning that links are security rules and that a link crosses numbers, and writes everything in one go. Nothing is written before it. The links, with their wake mark, go through the same code as the command `admin link add|remove`. Proposal (0050, built, not tried in a browser): the summary has one line per follow mark that changes, and one warning when a mark is turned on, saying that the lower Hermes will do what the higher one asks without waiting for its operator, that whoever misleads the higher Hermes reaches the lower one, and that a connector of an older version keeps treating the notes as data. The publish refuses a follow mark on a link that goes sideways or up or has no wake mark; a card or a level that moves so that a marked arrow no longer goes down drops the mark, and the page says so. The mark is set only here and by `org publish`; `admin link add` cannot set it and removes it from a link that has it.
- **Stale:** if the chart changed meanwhile (another admin, a command, a Hermes created or removed), nothing is published, the page shows the fresh state and keeps the draft to apply again.
- **Empty:** "No Hermes yet. Connect the first one in the Hermes tab." With no level, the draft starts with one level holding every Hermes.
- **Without script:** three read-only lists (levels, Hermes, links) and a sentence saying that editing needs the script.
- **Live refresh:** the tab refreshes the states like the others, but pauses while there is a draft.
- **Narrow screens:** the view starts as a list; the map scrolls sideways.
- **Does not do:** edit accesses to numbers (the card shows them and points to Accesses); show or edit any WhatsApp waking setting (the drawer no longer has a line for it; 16/11/2026, [decision 0049](decisoes/0049-acordar-pelo-whatsapp-so-pelo-verbo.md), which removed the switch and the command `admin wake`); put people on the chart; put a Hermes in two levels; tell any Hermes about levels, roles or kinds (the stage text of the tab says they are only shown here, and that a note is data except on a link with the follow mark, whose notice asks the receiver to follow it in that run; [decision 0050](decisoes/0050-etapa-b-marca-de-seguir.md), proposal). The command twin is `arauto-hub admin org show` and `org publish`.
- **Not guaranteed:** the draft is lost if the panel session ends before Publish; Publish does not ask for the passkey again (the note on top of this file covers it).
