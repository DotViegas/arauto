# 0040 — The panel inside the hub: its own port, entry by passkey and the first screens

**Status:** proposal · **Date:** 12/11/2026, with the code of the first delivery of the panel in
the hub (read-only screens); 13/11/2026, with the second (connecting a Hermes), the third
(accesses and revoking), the screens in nine languages, the entry by passkey and the lifecycle
of the installation code of the first access (design approved by the maintainer on 13/11/2026)

## Context

The [0033](0033-escopo-do-mvp.md) left the panel for after the MVP: administration is by command,
on the server. The [0019](0019-painel-com-passkey.md) says how it must be when it exists: login by
passkey, and, with the panel session alone, one can only see. On 12/11/2026 the maintainer asked
for the panel and approved the design of the screens.

This decision records what the deliveries of the panel do, what the code chose where the texts
are silent and where it moves away from them. Everything was tested only by the automatic tests,
against a test hub, with a passkey authenticator written for the tests: the panel never ran behind
a proxy, never connected a real Hermes, and **no real browser, phone or password manager has
registered a passkey in it yet**.

## Asked by the maintainer

- **On 12/11/2026:** the panel, and the design of its screens.
- **On 13/11/2026: the text of the screens is in English,** like everything else that is versioned
  ([0016](0016-idioma.md)). The prototype that was approved was in Portuguese; the screens kept its
  structure and changed its words.
- **On 13/11/2026: the screens speak the nine languages of the README.** English is the source;
  the other eight are translations of it.
- **On 13/11/2026: the English wording of the 02 was approved,** and with it the text of the
  block in English.
- **On 13/11/2026: the panel is entered with a passkey, and only with one.** An installation is
  to be easy for a person and for an agent: everything that is configured can be done on the
  screens and by command, and a new hub needs no command before its first start.
- **On 13/11/2026: a first access that is found without effort.** The code of a hub that never had
  a passkey has no time limit and serves once; the access instructions are meant to stay last in
  the output of the hub, where the newest lines are (the Logs of Easypanel 2.38.0 were observed to
  list the newest line last, with no search box); the entry shows only what works in the
  state of the hub; and the Open button of Easypanel is meant to land on the panel (it points to
  the primary domain: make the panel's domain the primary one). The maintainer also approved the new text of
  principle 2 of the `CLAUDE.md` and the note in the 04 about the one secret the hub writes in its
  log.

The first delivery entered by a code alone, with a session that could act, and kept the panel
reachable only by tunnel. That was a proposal of that delivery, recorded here as if it had been
chosen by the maintainer; it was replaced on 13/11/2026 by what is below.

## Decision

### Where the panel lives

- **Inside the hub, on a third port.** It is not another program: it uses the same functions that
  `arauto-hub admin` uses, with the same check of the organization.
- **Off by default.** The panel only exists with the two variables: `ARAUTO_LISTEN_PANEL` (where
  the port listens) and `ARAUTO_PANEL_URL` (the address the admin opens in the browser, with the
  same rules as `ARAUTO_PUBLIC_URL`). One without the other prevents `serve` from starting, and the
  address of the port cannot be that of either of the other two.
- **Each port only answers what is its own.** No address of the panel exists on the port of the
  connectors or on the internal one, and no address of theirs exists on that of the panel
  ([03-arquitetura.md](../03-arquitetura.md), section 4: the login of the admin never travels to
  the port of the connectors).
- **The request has to name the panel.** The `Host` header is that of `ARAUTO_PANEL_URL`; any
  other receives 421 before any read.
- **Pages written by the program itself, with no template library.** The markup is fixed text of
  the program; every value passes through a single escape function. The 03 foresaw "automatic
  escape templates": the escape remains mandatory, it just does not come from a library.
- **One script only, which is a file of the panel.** No page carries a script written inside it,
  and the content policy sent in every response (`default-src 'none'`, script, sheet and fonts only
  from the panel itself, `form-action 'self'`, `frame-ancestors 'none'`) prevents the browser from
  running another. Only the pages of the guide load the file. It helps the screen (copying the
  block, the animation, the countdown of the deadline): it does not write markup, does not read a
  cookie and does not call any address. Without it, each step keeps working, because each step is
  a page of the hub.
- **The style sheet, the fonts, the script and the mascot go inside the program.** The fonts are
  free, with the license beside them.

### The entry

- **A passkey opens the panel, and nothing else does.** The ceremonies are the ones of WebAuthn,
  done by a library (`webauthn_rp`, version 0.3.0, pure Rust) and never by code of our own. The
  person has to be verified by the device (fingerprint, face or PIN) at the registration and at
  every entry. The address the passkey is bound to is the one of `ARAUTO_PANEL_URL`: its name is
  the identifier of the party, and its origin (scheme, name and port) is the only one accepted.
- **The hub keeps what checks a passkey:** its public key and its counters, as the library
  encodes them. Nothing kept can sign in the place of the person.
- **The installation code only opens the registration of a passkey.** It has 20 bytes from the
  system generator and the hub keeps only its SHA-256, which it erases when the code is spent or
  cancelled. It serves once, and issuing a code of either kind cancels the unspent one in the
  same write, together with the panel sessions that code opened and that never reached the whole
  panel. It comes from two places:
  - **the start of the hub** (a *start code*), while the first access was never done: the hub
    writes it in the output of its start, with no deadline, and writes the same block again
    every 10 minutes while the code is unspent. Each start issues a new one, so the block with
    the latest time is the one that works. It is the one secret the hub writes there, on purpose
    ([07-telas.md](../07-telas.md), T0): whoever reads that output on the server of the hub usually
    already holds the database and the master key; a copy of the output taken outside does not (see
    What is not guaranteed). It goes straight to the standard error of the start and not through
    the tracing lines of the log, and the clear code lives only in the memory of the task that
    writes the block;
  - **the command** `arauto-hub admin panel code` (a *command code*), valid for 10 minutes to be
    typed, in every state of the hub: the way back for who lost the passkey, or cannot open the
    log.
- **The first access is a durable mark.** It is written once, in the same write that keeps the
  first passkey of the organization, and never cleared: removing or losing every passkey later
  does not bring the start code back, and the way back is the command.
- **How far a panel session reaches.** A session opened by a code reaches one page: the
  registration of a passkey. Every other page leads to it and no form of the panel is taken from
  it. With a passkey registered in it, that same session reaches the whole panel; a session opened
  by a passkey reaches it from the start.
- **The code is typed, never shown by the panel,** and typing it does not spend it: it opens a
  panel session that reaches only the registration of a passkey and points at its code. The code
  is spent by keeping a passkey in that panel session, in the same write and under the lock of
  the row of the code, and only while the code is still unspent. Typing the same code again
  closes the panel session it opened before, so a lost page (a closed browser, 30 minutes idle, a
  restarted hub) needs only the latest code. It does not go in an address: opening a page never
  spends a code.
- **One sentence only for every refusal** of a code, whatever the reason (wrong, spent, replaced
  by another, or past its 10 minutes): the pages are byte for byte the same, so the refusal does
  not say why. A refused passkey has its own sentence.
- **The ceilings of attempts** are per client address (the address the hub takes the client to
  have), plus a larger one of everyone together, in separate buckets for the lookups of a code and
  for the challenges of a passkey. The map of addresses is bounded: when it is full, the global
  bucket is the one that counts. The ceiling does not protect the code, which has 160 bits: it
  keeps a flood away from the database and from the memory of the ceremonies, and one address
  cannot use up the allowance of the others.
- **The entry is by state, and it is the state of one organization:** the one the admin command
  resolves, found when the hub starts and kept in the state of the panel. With zero or several
  organizations, the entry shows the returning page with no sentence of the first access; no
  start code is issued and nothing is written where the block goes, only a line of the log says
  why.
  - **First access not done:** step 1 of three (code, passkey, check), with where to find the code
    (the Logs of the service in Easypanel, or `docker compose --env-file arauto.env logs hub`:
    the Docker recipe always passes `--env-file`), which block to take (the page says "Find the
    ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its
    code.", a wording that asks for no search, since the Easypanel viewer has no search box),
    and a field for the code. **There is no passkey form at all** on this
    page, and `POST /passkey/sign-in/start` answers 404 while the first access is not done.
  - **First access done:** the page of who is back, with the passkey first and, under a "Lost your
    passkey?" element, the form of the code and the command that issues one. The element comes
    open when the page carries a refusal of a code.
  - **Reading the entry writes nothing:** `GET /sign-in` only reads the durable mark.
  - **Step 2** is the registration of the passkey, in a panel session opened by a code. It shows
    the origin of `ARAUTO_PANEL_URL` and never the `Host` of the request. Leaving the page and
    typing the same code again goes back to it, because only the passkey spends the code. A
    registration with no valid panel session, or whose code is no longer unspent, shows the entry
    page of the current state with a message that the panel session ended. The script reaches that
    page too: when asking for the challenge finds no panel session (401), it sends the form with
    no credential. On a returning page the field of the code opens beside that message.
- **The first-access task writes the block, and no request does.** Started by `serve` after the
  panel listens, it writes the block straight to the standard error at its start and every 10
  minutes while its code is unspent and the first access is not done. Before each write it reads
  the database, and when the read fails it writes nothing. A code cancelled by a command code
  makes it write a "replaced" block once and stop; cancelled by another start, it stops in
  silence. The registration of the first passkey only signals the task, which then writes one
  "done" line and clears the code from its memory. Without a panel, `serve` writes one line that
  says so; when the start code could not be issued, one line that says so, with no error text.
  The two framed blocks are ASCII and have no line of fixed text over 80 columns (the address
  goes on a line of its own); the one-line notices (done, no panel, could not be issued) stay one
  line each and may be longer. The block carries the origin of
  `ARAUTO_PANEL_URL`, says "no time limit" and "latest", and never says minutes. The repeat is
  tested through a tick with the fake clock of the tests: there is no variable that shortens it.
- **The check window** (`/installation`), step 3, is a window over the list of Hermes that only
  reads and needs a panel session that reaches the whole panel. It comes from one function, and
  `arauto-hub admin check` prints the same rows in English. Rows: the address of the panel, the
  address of the connectors, the address the hub sees the visitor at (only in the panel: a
  forwarded address from a proxy that is not in `ARAUTO_TRUSTED_PROXIES`, or a private one with
  no trusted proxy, is flagged), the password of the database (replaced by the hub, not asked
  for, failed, or "later" before the first passkey when the replacement was asked for), the connector served, the data folder, and the WhatsApp engine (set or not,
  never its address or key). After the first passkey the panel goes there with `?welcome`, which
  adds the stepper and the greeting; later passkeys go back to the start. A footer link on the
  pages of the panel opens it at any time.
- **The root of the port of the connectors** answers `GET /` with 200 and a fixed text of three
  lines: that the address is for the connectors, that the panel has its own address (the one of
  `ARAUTO_PANEL_URL`) and a pointer for the person the Open button of Easypanel brought there.
  The code today still tells that person to point the connectors' domain to the port of the panel,
  which is wrong: it takes the domain away from the connectors, and the panel answers 421 on any
  host other than `ARAUTO_PANEL_URL`. The right advice is to make the panel's domain the primary
  one, or to open the panel's address directly (`deploy/easypanel/README.md`, step 2.4). The text
  is outward-facing and waits for the maintainer to approve the change (pending decision 5). It
  names no host, no port and no version.
  Every other path and method stays 404, and `/` on the internal port stays 404.
- **The check of `Host` stays the first thing on the port of the panel:** 421 before any read,
  any lookup or any counted attempt. The addresses the hub uses come only from `ARAUTO_PANEL_URL`
  and `ARAUTO_PUBLIC_URL`.
- **The challenge of a ceremony lives in the memory of the hub,** for 5 minutes and one answer,
  with at most 256 of each kind at a time. The script of the page asks the panel for it, hands it
  to the browser and sends back what the browser answers; the content policy lets the script call
  the panel itself and nothing else.
- **Pieces used:** the library above, SHA-256 with a fixed prefix for each use, the system
  generator, and a cookie `HttpOnly` and `SameSite=Strict` (with `Secure` and the prefix `__Host-`
  when the address of the panel is `https`). No cipher of our own.
- **The first passkey can replace the password of the database** with one the hub draws and
  nobody sees, when the installation asks for it ([0041](0041-senha-do-banco-sorteada-pelo-hub.md)).
- **A hub that starts on an empty database installs itself:** it does what `arauto-hub init`
  does, by the same function and with the same refusals. A database that has anything in it is
  not touched by this, and a migration is never applied by the start.

### The panel session

- **The identifier** has 32 bytes from the system generator and lives only in the cookie. The hub
  keeps its SHA-256, and erases that digest when it closes the panel session.
- **It lasts 12 hours, or 30 minutes without use,** as the [07-telas.md](../07-telas.md) foresaw.
- **The time without use stays in the memory of the hub, not in the database,** so that a read
  request writes nothing. Consequence: when the hub restarts, every panel session ends, and the
  admin enters again with the latest code: the same one if the hub did not restart, the one of
  the newest block if it did.
- **Each request checks the panel session in the database** (digest, state and validity), with no
  intermediate memory of what was authorized before.
- **A form is only accepted with the `Origin` of the panel.** Without it, or with another, the
  answer is 403 before the form is read.
- **In the audit:** the entry (`panel.signed_in`, by code or by passkey) and the registration of
  a passkey (`panel.passkey_registered`) are security events; the issued code
  (`panel.code_issued`) and the exit (`panel.signed_out`) are common events. The actor is the local
  admin; where the command writes the system user, the panel writes the fixed word `panel`.
- **Purge:** the rows of the codes and of the panel sessions leave 30 days after the moment they
  stopped being valid. What each one did remains in the audit.

### The read-only screens

- **Hermes, WhatsApps and Accesses, with the alert banner at the top.** They show what
  `arauto-hub admin` shows, plus what only the running hub knows: which Hermes are connected
  now.
- **What the screens never show:** installation code, verification code, address or key of the engine,
  name of the engine session and message content.

### Connecting a Hermes

The guide of the [02-como-funciona.md](../02-como-funciona.md), section 1, in a window over the
list of Hermes, with the path of the stages at the top: name, block, code, done.

- **The rule of each action stays in one place only.** Creating the line, issuing the invite,
  approving and denying are the functions that `arauto-hub admin` calls, with the same checks, the
  same audit and the same mark. Only the order of the screens belongs to the panel.
- **The invite appears once,** in the answer of the form that issued it. No address opened by a
  read issues, shows or spends an invite, and the database keeps storing only the digest.
- **The text of the block is the one proposed in the 02,** word for word, in English; the panel
  only fills in the fields. A test compares the block of the panel with the text of the document
  and fails when one changes without the other. Where the 02 writes `arauto-connector-{VERSION}`, the name of the file the hub serves
  goes, with the SHA-256 calculated from the bytes served.
- **Without a connector to serve, the panel makes no block** and issues nothing: it says to use
  the command. With more than one connector for Linux in the folder, the block uses the first by
  name.
- **The verification code is typed and never shown,** not even back to whoever got it wrong.
- **Each stage lights up when the hub observes it:** invite issued, device used the invite,
  request approved, connector confirmed the first credential. The page that waits asks again by
  itself, and that question only reads.
- **In the reconnect, the answer about the last use applies to the use that was on the screen.**
  If the current device was used while the admin was answering, nothing is approved and the new use
  is shown.
- **Closing the guide does not cancel the invite.** It expires by itself, in 15 minutes, or is
  canceled by another invite; the line of the Hermes leads back to the guide.

### Accesses and revoking

- **Each cell of the matrix opens the editor of that access,** in a window: where it applies (all
  the chats or only the chosen ones), the four verbs, the wait before waking and "may start
  chats". A sentence says what the access means, and changes while one marks.
- **Saving is the `access grant` of the command, followed by the `wake-wait`.** With no verb, in
  "all the chats", it is the `access remove`. The rules are those of the command (being woken
  requires reading; starting chats requires sending and all the chats): the panel only says them
  in the words of the screen, and what the script greys out is checked again on saving.
- **They are two writes, not one:** if the wait fails after the access is written, the screen says
  that something failed and the matrix shows what remained.
- **What the editor does not do yet: choosing the chats** of the scope "only chosen chats". The
  chat lines continue through the `access chat` of the command; the screen shows how many exist.
  The reason is that choosing a chat depends on the identifiers of the engine, not yet seen in a
  real engine.
- **Revoking asks first.** The line of the Hermes leads to a window that says what revoking
  closes; only its form revokes, through the `revoke` of the command.
- **The address that says "Saved" only reads:** it carries the Hermes and the number, and the page
  writes the access as it is in the database at that moment.

### The languages of the screens

- **Nine languages, the ones of the README:** English, Portuguese, Spanish, French, German,
  Russian, Hindi, Simplified Chinese and Japanese.
- **Which one a page comes in:** the one the person chose in the panel; without a choice, the
  first one the browser prefers (`Accept-Language`) that the panel speaks; without that, English.
- **The choice is a cookie that holds the tag of a language and nothing else** (`arauto-panel-lang`,
  one year). It is set by a form, with the same `Origin` check of every form, and needs no panel
  session: the entry screen has the same form. Nothing about it is written in the hub. A value
  that is not one of the nine tags chooses nothing, in the form and in the cookie.
- **English is the source, inside the code.** Each sentence of a screen is marked where it is
  written, and each language has one file with the pairs "English sentence, translation". A
  sentence without a translation is shown in English.
- **A translation is text, never markup.** It passes through the same escape as any value; what a
  sentence carries inside it (a name, a number, a command, a deadline) is put in by the program, in
  the place the translation marks.
- **A test reads the code and fails** when a sentence of a screen is not marked, when a language
  lacks one, translates one that no screen says, changes what the sentence carries, or brings
  markup. It also fails when two answers offered side by side (revoke and cancel, approve and deny)
  read the same in some language.
- **The script has no sentence of its own:** what it writes on the screen comes from the page, in
  the language of the page.
- **What does not change with the language:** the names (Hermes, WhatsApps, the name of each
  Hermes and of each number), the commands, the codes like `AR-102`, the text of the block, what
  `arauto-hub admin` prints, the audit and the log of the hub.

## What is not guaranteed

- **The eight translations were written by a language model and were not reviewed by native
  speakers of each language.** They were seen only in photos of some screens of a test hub. A
  sentence may be wrong or read badly, and the English one is the one that counts.
- No language written from right to left is in the list, and the screens were never drawn for one.
- The installation code does not identify a person. Whoever runs the command on the server of the
  hub already has the database and the master key; whoever reads the output of its start usually
  does too, except from a copy taken outside (see the start code below). Every passkey belongs to
  the one "local admin".
- **The start code is as secret as the log of the hub.** It has no deadline: whoever reads the
  output of the start, in any copy (the log storage, a log shipper, a picture, a pasted excerpt),
  can register the first passkey until the admin does. Whoever can read those logs inside
  Easypanel usually controls the service already; a copy taken outside does not.
- **The log keeps many copies of the start code.** The hub writes it again every 10 minutes, so
  the log storage holds many of them. They all die together at the first passkey or at the next
  start.
- **Between typing the code and registering the passkey, the code still serves.** Whoever sees it
  in that interval can open a registration too, and the first passkey registered wins.
- **The entry tells an anonymous visitor whether the hub already had its first access.** The
  page differs by state, on purpose: it is what shows the person what to do.
- **Two versions of the hub must not serve the same database at once.** The mark of the first
  access is written by the version that has it. A hub of an older version that kept a passkey
  while a newer one served the same database would leave a passkey with no mark. The start
  repairs this by itself (a passkey of the organization, in any state, counts as "the first
  access was done", and the mark is set from the oldest one, with no code issued; a command
  code that is unspent stays accepted, and a start code left from before is cancelled), but an
  upgrade is still a stop of the old hub and then a start of the new one, never an overlap.
- **The heading of a block carries the date** (`YYYY-MM-DD HH:MM:SS UTC`), so that "the block
  with the latest time" stays right across midnight and across days.
- **A restart cancels the code in force.** A code copied before it is refused, and the refusal
  does not say why. While it is valid, the code lives in the memory of the hub process.
- **The ceilings of attempts are per address, plus one for everyone.** Behind a proxy that is not
  trusted every request carries the address of the proxy, and the ceiling is shared again. When
  the hub cannot tell the client address behind a trusted proxy (a malformed rightmost item of
  `X-Forwarded-For`), the attempt is counted under the proxy. Enough addresses together can still
  fill the ceiling for everyone, and while they do nobody types a code or signs in: a flood can
  delay the entry. It is also stated in [05](../05-modelo-de-ameacas.md).
- **The first access ends with a person:** registering a passkey needs a phone or a computer with a
  fingerprint, a face or a PIN.
- **What Easypanel does is only partly checked:** on Easypanel 2.38.0 (13/11/2026) the Logs
  screen was observed to list the oldest line first, with no search box, the Open button was
  observed to point to the primary domain, and a "Make primary" star and a Console button exist
  ([referencia/easypanel-fatos.md](../../referencia/easypanel-fatos.md)). Not checked: moving the
  primary domain, what the console opens, the colors of the Logs and the templates
  ([deploy/easypanel/README.md](../../deploy/easypanel/README.md)).
  A panel on a name of the Easypanel zone also depends on the DNS of the provider of Easypanel and
  on the names of the project and of the service: for real use, a name of your own.
- **Until the first passkey, the database of the Easypanel recipe keeps the password written in
  its `compose.yaml`.**
- **A stolen panel session acts while it is valid** (12 hours, or 30 minutes without use). With it
  one can see everything the screens show, change any access, revoke a Hermes and reconnect a
  Hermes on a device of whoever stole it, which then inherits its accesses. The
  [0019](0019-painel-com-passkey.md) asks for the passkey again on every action that changes
  something: that is not built.
- **The passkey was only exercised by an authenticator written for the tests.** How each browser,
  phone, security key and password manager answers was not seen. Known limits of the library: it
  accepts no attestation other than "none" or a self-signed one, it refuses an answer whose backup
  state changed since the registration, and it refuses a counter that does not grow once it left
  zero.
- **The challenges of the ceremonies are drawn by the library,** from a generator seeded by the
  system generator, not by a direct call to it as the rest of the project does.
- A passkey is bound to the address of the panel: change `ARAUTO_PANEL_URL` and every passkey
  stops working, and the way back is an installation code from the command (moving to another
  name means registering another passkey).
- Whoever reaches the port of the panel can ask for challenges: 256 of them, unanswered, keep
  anyone from entering for up to 5 minutes. The ceilings slow that down and do not prevent it.
- The panel depends on the TLS of whoever installs. Without TLS it is only accepted on
  `localhost`, for use by tunnel, and then the cookie does not carry `Secure`. A browser only
  makes a passkey over TLS or on `localhost`.
- In a container, the port of the panel listens on all interfaces: whoever is on the same network
  as the hub, like the WhatsApp engine, reaches the entry screen. What holds there is the
  single-use code.
- The block passes through the screen and through the clipboard of whoever copies it. The invite
  inside it is valid for 15 minutes and one use.

## Pending decisions, for the maintainer

1. **The text of the block.** The panel uses the one proposed in the 02. The command does not
   print that text: it prints the address of the hub, its key, the invite and the same command
   lines, with no sentence addressed to the Hermes. It remains to make the command print the
   block of the 02, or to say that they are two on purpose.
2. **The passkey on every action,** as the 0019 asks, or only at the entry, as it is built.
3. **More than one admin.** Every passkey belongs to the one local admin; the 0019 foresees a
   second admin giving a passkey to the first.
4. **Three lines printed only by `arauto-hub admin check`,** in English, outside the approved
   texts: "Not set in this environment.", "Only the running hub sees it: open the panel and the
   check window." and "Replaced when the first passkey is registered.". The window shows no
   sentence for the state "later" of the database password. They wait for the maintainer to
   approve them (and to give them a place in the catalog, if they are to be translated).
5. **A text that conflicts with Easypanel.** The fixed text at the root of the connectors' port
   tells the person to point the connectors' domain to the panel (wrong, see above). Proposed:
   "If the Open button of Easypanel brought you here, make the panel's domain the primary one, or
   open the panel's address directly.". It waits for the maintainer. (Step 1 of the entry had the
   same kind of conflict, "Search for INSTALLATION CODE", with a viewer that has no search box;
   the maintainer approved its new wording on 13/11/2026 and it is in the 9 languages.)
6. **A sentence that says more than is guaranteed.** Step 1 of the entry says "Only whoever looks
   after the server sees that log: that is how the hub knows it is you.", but a copy of the log
   taken outside (log storage, a log shipper, a picture, a pasted excerpt) gives the code to
   someone who holds nothing. Proposed: "Whoever looks after the server sees that log: keep it to
   yourself.", in the 9 languages. It waits for the maintainer.

## Consequences

- The 0033 gains the note that the panel started before the end of the MVP, and the 0019 the note
  of what of it is built. The
  [0028](0028-painel-fechado-por-padrao.md) does not change.
- The hub now has an administration address on the network, when whoever installs turns it on. The
  Docker guide publishes the port of the panel only on `127.0.0.1`; the Easypanel recipe
  (proposal) puts it on a domain of its own, behind the passkey.
- A database that already has data needs the new migrations before `serve` starts
  (`arauto-hub migrate --i-have-a-backup`).
- **Fact of 13/11/2026 (the status of this decision stays proposal).** The lifecycle of the
  installation code, the entry by state, step 2, the check window, the command
  `arauto-hub admin check` (the window and the command read their facts from one function, and a
  test compares their rows), the one refusal sentence, the ceilings of attempts and the root of
  the port of the connectors are in the code and passed the automatic tests (the `lib`, `panel`,
  `panel_canary`, `repo` and `channel` targets of `arauto-hub`; the whole workspace suite is run
  by the maintainer's model at the end). The ceilings are 30 a minute per address and, for
  everyone together, 600 for codes and 120 for passkey challenges; an IPv6 address counts by its
  /64; the map holds 4096 addresses. **Not seen:** no real browser, no real Easypanel and no real
  proxy has run any of it, and the entry never ran over a real network.
- **Proposal, not proved by a test:** the parts of what Easypanel does that
  `referencia/easypanel-fatos.md` marks as to check (moving the primary domain, the console, the
  templates and the colors of the Logs), and how each browser and password manager answers a passkey.
- The name "entry code", a proposal of the first delivery, left the glossary: the code is the
  installation code of the 0019.
- Alternatives considered: a separate program for the panel (another database credential and
  another surface to protect); a template library (automatic escape, but one more dependency);
  keeping the last use of the session in the database (it survives the restart of the hub, but is
  a write in a read request); password (the 0019 already discarded it); a panel with no script at
  all (the block would not be copied with one click, which is what the approved design asks for).

## Change of 14/11/2026

The guide to connect a Hermes shows the connection sentence first and the block behind "Show the command instead"; step 2 is called "Message". The instruction page and `/llms.txt` are served on the port of the connectors, not on the panel port. The maintainer asked for the link and the page and approved the design on 14/11/2026. Decision [0042](0042-frase-de-conexao-e-pagina-de-instrucoes.md). Nothing else in this decision is reopened. The design was not tried on a real Hermes yet.
