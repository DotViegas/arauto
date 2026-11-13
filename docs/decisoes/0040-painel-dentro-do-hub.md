# 0040 — The panel inside the hub: its own port, entry by code and the first screens

**Status:** proposal · **Date:** 12/11/2026, with the code of the first delivery of the panel in
the hub (read-only screens); 13/11/2026, with the second (connecting a Hermes) and the third
(accesses and revoking)

## Context

The [0033](0033-escopo-do-mvp.md) left the panel for after the MVP: administration is by command,
on the server. The [0019](0019-painel-com-passkey.md) says how it must be when it exists: login by
passkey, and, with the panel session alone, one can only see. On 12/11/2026 the maintainer asked
for the panel and approved the design of the screens.

The passkey has not been built yet. This decision records what the first three deliveries of the
panel do, what the code chose where the texts are silent and where it moves away from them.
Everything was tested only by the automatic tests, against a test hub: the panel never ran on a
server, in a container or behind a proxy, and never connected a real Hermes.

## Decided by the maintainer, on 12/11/2026

- **The entry is by code while the passkey does not exist.**
- **The panel session opened by that code can act** (connect a Hermes, pair, give access). This
  changes the 0019, which asked for the passkey on each action, and is noted in it.
- **In this period the panel stays closed:** its port is only reached on the hub's own machine, by
  tunnel. The passkey comes back before the panel is opened to the internet.
- **On 13/11/2026: the text of the screens is in English,** like everything else that is versioned
  ([0016](0016-idioma.md)). The prototype that was approved was in Portuguese; the screens kept its
  structure and changed its words.

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

- **Entry code.** The command `arauto-hub admin panel code`, run on the hub's machine, draws
  20 bytes from the system generator and shows the code once. The hub keeps only its SHA-256. It is
  valid for 10 minutes and one entry; issuing another cancels the previous one.
- **The code is typed, never shown by the panel,** and is only spent by a submitted form (`POST`).
  It does not go in an address: opening a page never spends a code.
- **One sentence only for every refusal** (wrong code, expired, already used), and a ceiling of
  30 code lookups per minute, of everyone together. The ceiling does not protect the code, which
  has 160 bits: it keeps a flood away from the database.
- **Pieces used:** SHA-256 with a fixed prefix for each use, the system generator, and a cookie
  `HttpOnly` and `SameSite=Strict` (with `Secure` and the prefix `__Host-` when the address of the
  panel is `https`). No cipher of our own.

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
- **In the audit:** the entry is a security event (`panel.signed_in`); the issued code
  (`panel.code_issued`) and the exit (`panel.signed_out`) are common events. The actor is the local
  admin; where the command writes the system user, the panel writes the fixed word `panel`.
- **Purge:** the rows of the codes and of the panel sessions leave 30 days after the moment they
  stopped being valid. What each one did remains in the audit.

### The read-only screens

- **Hermes, WhatsApps and Accesses, with the alert banner at the top.** They show what
  `arauto-hub admin` shows, plus what only the running hub knows: which Hermes are connected
  now.
- **What the screens never show:** entry code, verification code, address or key of the engine,
  name of the engine session and message content.

### Connecting a Hermes

The guide of the [02-como-funciona.md](../02-como-funciona.md), section 1, in a window over the
list of Hermes, with the path of the stages at the top: name, block, code, done.

- **The rule of each action stays in one place only.** Creating the line, issuing the invite,
  approving and denying are the functions that `arauto-hub admin` calls, with the same checks, the
  same audit and the same mark. Only the order of the screens belongs to the panel.
- **The invite appears once,** in the answer of the form that issued it. No address opened by a
  read issues, shows or spends an invite, and the database keeps storing only the digest.
- **The text of the block is the one proposed in the 02,** word for word; the panel only fills in
  the fields. Where the 02 writes `arauto-connector-{VERSION}`, the name of the file the hub serves
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

## What is not guaranteed

- The entry code does not identify a person. Whoever runs the command already has the database and
  the master key; every panel session belongs to the "local admin".
- **A stolen panel session acts while it is valid** (12 hours, or 30 minutes without use). With it
  one can see everything the screens show, change any access, revoke a Hermes and reconnect a
  Hermes on a device of whoever stole it, which then inherits its accesses. It is the risk accepted
  on 12/11/2026, and the reason the panel stays closed until the passkey exists.
- The panel depends on the TLS of whoever installs. Without TLS it is only accepted on
  `localhost`, for use by tunnel, and then the cookie does not carry `Secure`.
- In a container, the port of the panel listens on all interfaces: whoever is on the same network
  as the hub, like the WhatsApp engine, reaches the entry screen. What holds there is the
  single-use code.
- The block passes through the screen and through the clipboard of whoever copies it. The invite
  inside it is valid for 15 minutes and one use.

## Pending decisions, for the maintainer

1. **The name "entry code".** It entered the glossary as a proposal. The alternative is to widen
   the "installation code".
2. **The text of the block.** The panel uses the one proposed in the 02, which is still in
   Portuguese: its English wording waits for the maintainer's approval, with the rest of the 02.
   The command prints another, in English, with the same command lines. It remains to choose one,
   or to say that they are two on purpose.
3. **The [04-seguranca-conexao.md](../04-seguranca-conexao.md)** says, in two tables, that the
   login of the panel is by passkey. It was not changed: it changes only with approval.

## Consequences

- The 0033 gains the note that the panel started before the end of the MVP, and the 0019 the note
  that the panel session acts while the passkey does not exist. The
  [0028](0028-painel-fechado-por-padrao.md) does not change.
- The hub now has an administration address on the network, when whoever installs turns it on. The
  installation guide publishes the port of the panel only on `127.0.0.1`.
- A database that already has data needs the new migration before `serve` starts.
- Alternatives considered: a separate program for the panel (another database credential and
  another surface to protect); a template library (automatic escape, but one more dependency);
  keeping the last use of the session in the database (it survives the restart of the hub, but is
  a write in a read request); password (the 0019 already discarded it); a panel with no script at
  all (the block would not be copied with one click, which is what the approved design asks for).
