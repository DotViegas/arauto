# 0040 — The panel inside the hub: its own port, entry by code and the first screens

**Status:** proposal · **Date:** 12/11/2026, with the code of the first delivery of the panel in
the hub (read-only screens); 13/11/2026, with the second (connecting a Hermes), the third
(accesses and revoking), the screens in nine languages and the entry by passkey

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
  system generator and the hub keeps only its SHA-256; it serves once, and issuing another cancels
  the previous one. It comes from two places:
  - **the start of the hub,** while no admin has a passkey: the hub writes one in the output of
    its start, valid for 30 minutes. It is the one secret the hub writes there, on purpose
    ([07-telas.md](../07-telas.md), T0): whoever reads that output already holds the database and
    the master key. It goes straight to the output and not through the log lines;
  - **the command** `arauto-hub admin panel code`, valid for 10 minutes: the way back for who
    lost the passkey.
- **How far a panel session reaches.** A session opened by a code reaches one page: the
  registration of a passkey. Every other page leads to it and no form of the panel is taken from
  it. With a passkey registered in it, that same session reaches the whole panel; a session opened
  by a passkey reaches it from the start.
- **The code is typed, never shown by the panel,** and is only spent by a submitted form (`POST`).
  It does not go in an address: opening a page never spends a code.
- **One sentence only for every refusal** of a code and one for every refusal of a passkey, and a
  ceiling of 30 lookups per minute, of everyone together, shared by the code and by the challenge
  of an entry. The ceiling does not protect the code, which has 160 bits: it keeps a flood away
  from the database and from the memory of the ceremonies.
- **The challenge of a ceremony lives in the memory of the hub,** for 5 minutes and one answer,
  with at most 256 of each kind at a time. The script of the page asks the panel for it, hands it
  to the browser and sends back what the browser answers; the content policy lets the script call
  the panel itself and nothing else.
- **Pieces used:** the library above, SHA-256 with a fixed prefix for each use, the system
  generator, and a cookie `HttpOnly` and `SameSite=Strict` (with `Secure` and the prefix `__Host-`
  when the address of the panel is `https`). No cipher of our own.
- **A hub that starts on an empty database installs itself:** it does what `arauto-hub init`
  does, by the same function and with the same refusals. A database that has anything in it is
  not touched by this, and a migration is never applied by the start.

### The panel session

- **The identifier** has 32 bytes from the system generator and lives only in the cookie. The hub
  keeps its SHA-256, and erases that digest when it closes the panel session.
- **It lasts 12 hours, or 30 minutes without use,** as the [07-telas.md](../07-telas.md) foresaw.
- **The time without use stays in the memory of the hub, not in the database,** so that a read
  request writes nothing. Consequence: when the hub restarts, every panel session ends, and the
  admin enters again with another code.
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
- The installation code does not identify a person. Whoever runs the command, or reads the
  output of the start of the hub, already has the database and the master key; every passkey
  belongs to the one "local admin".
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
  stops working, and the way back is an installation code.
- Whoever reaches the port of the panel can ask for challenges: 256 of them, unanswered, keep
  anyone from entering for up to 5 minutes. The ceiling of 30 a minute slows that down and does
  not prevent it.
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

## Consequences

- The 0033 gains the note that the panel started before the end of the MVP, and the 0019 the note
  of what of it is built. The
  [0028](0028-painel-fechado-por-padrao.md) does not change.
- The hub now has an administration address on the network, when whoever installs turns it on. The
  installation guide publishes the port of the panel only on `127.0.0.1`.
- A database that already has data needs the new migrations before `serve` starts
  (`arauto-hub migrate --i-have-a-backup`).
- The name "entry code", a proposal of the first delivery, left the glossary: the code is the
  installation code of the 0019.
- Alternatives considered: a separate program for the panel (another database credential and
  another surface to protect); a template library (automatic escape, but one more dependency);
  keeping the last use of the session in the database (it survives the restart of the hub, but is
  a write in a read request); password (the 0019 already discarded it); a panel with no script at
  all (the block would not be copied with one click, which is what the approved design asks for).
