# 0042 — The connection sentence and the instruction page

**Status:** proposal · **Date:** 14/11/2026. Built in the hub; **not tried on a real Hermes yet.**

## Context

On 14/11/2026 the first real attempt to connect a production Hermes, with the block alone, was
refused by that Hermes. The cause is not known (see
[referencia/hermes-fatos.md](../../referencia/hermes-fatos.md)). One hypothesis, not proved: a
pasted procedure with a command looks like an attack, and a short message from the operator that
points to an explanation looks like a request.

## Asked and approved by the maintainer

The maintainer asked for the link and the page, and approved the design below on 14/11/2026, with
three answers: the page in English, the request in the language Arauto is showing, and that it be
done immediately and in production.

## Decision

- **The panel shows a connection sentence first.** Short, in the first person, in the language the
  panel is showing (nine languages). It carries, as labelled values: the address of the instruction
  page, the hub address, the hub key, the SHA-256 of the connector and the invite, and asks for the
  verification code only. Beside it, a hint: send it as a new message, not as a reply to a notice;
  if Hermes asks to confirm, answer in your own words. The labels are translated; the values are
  appended by code, never inside a translated string.
- **The block stays, unchanged,** behind "Show the command instead" and with `--command` in the
  admin command.
- **An instruction page per invite,** `GET /v1/join/{handle}` on the port of the connectors, in
  English only, as Markdown text. The handle is not the invite: 16 bytes from the system generator,
  born with the invite, kept in the database only as a digest, erased with the invite. The page is a
  fixed text compiled into the hub, with strict fields (host, Hermes name, UTC deadline, version,
  file name and, when it is known at build time, the public source address). It never holds the invite, the hub key or the connector digest;
  it tells the agent to recognize those values by their form. It says it is not an order: the
  operator's sentence is. It never uses a system header, a stamp, "authorized by", urgency, "without
  asking for approval", `curl` piped to a shell, `sudo`, a shortener or an encoded piece.
- **It serves** while the invite is valid or its request still waits for the code. Otherwise, one
  byte-identical 404. A GET only reads.
- **`GET /llms.txt`** on the same port: a short fixed text.
- **Nothing else changes:** the same `join`, the same invite (15 minutes, one use), the same
  SHA-256 check and the same verification code typed by the person.
- **Principle 6 of the `CLAUDE.md`** gains the sentence on the fixed page (approved wording);
  [02](../02-como-funciona.md) swaps the block for the sentence, keeping the block as the fallback.
  [04](../04-seguranca-conexao.md) was **not** changed: see the pending decisions.

## What is not guaranteed

- **That a Hermes accepts it.** Not tried on a real Hermes. The cause of the first refusal is
  unknown, so this design may fail the same way.
- **That the address stays secret.** The handle can leak through a preview service or a proxy log.
  It reveals the name of the Hermes, the deadline and, by the switch from 200 to 404, roughly when
  the request ended. It does not reveal the invite.
- **That a Hermes reads only what the operator meant.** The page is text read by an LLM; a
  look-alike page can be written by anyone ([05](../05-modelo-de-ameacas.md), rows 6a to 6c). The
  defense is structural: no access without the invite from the authenticated panel and the code
  typed by the admin.
- **That a translated sentence keeps its force.** The translations were not read by native
  speakers.

## Pending decisions, for the maintainer

1. Whether, after a real test, the sentence or the block becomes the default for good.
2. The text of [04](../04-seguranca-conexao.md): C1 and C2 and the line "The hub does not send free
   text" describe the block as the only carrier and no hub text read by the LLM. Drafts of the changes were given to the
   maintainer for approval; the file is untouched.
3. The `repository` metadata of the project, so the page can name the public source.

## Consequences

- The router of the connectors' port gains two addresses and a ceiling of lookups.
- A new column in the invite table holds the digest of the handle ([06](../06-modelo-de-dados.md)).
- A proxy in front of the hub must pass `/v1/join/` and `/llms.txt`
  ([deploy/proxy](../../deploy/proxy/README.md)).
- The test on a real Hermes goes to step 3.0 of [primeiro-teste](../../deploy/primeiro-teste.md).

## Change of 14/11/2026: three systems

**Status of this change: proposal.** Compiled facts below are stated by the maintainer's assistant
and recorded in [referencia/hermes-fatos.md](../../referencia/hermes-fatos.md).

### What was asked and approved

A Hermes on Windows 11 could not connect: the page only offered the Linux file. The maintainer
approved, on 14/11/2026, that the connector is served for Linux, Windows and macOS, and that the
connection sentence carries one digest line per system in place of the single line.

### The design

- **Four files, fixed names.** `arauto-connector-linux-x86_64` (`linux x86_64`),
  `arauto-connector-windows-x86_64.exe` (`windows x86_64`), `arauto-connector-macos-aarch64`
  (`macos arm64`) and `arauto-connector-macos-x86_64` (`macos x86_64`). The hub recognises a
  connector only by these exact names in its downloads folder, in this order. Another name with the
  prefix is still served by `/v1/download` but is not a connector of the sentence. Any subset may
  be present; with none, nothing is issued.
- **The image builds the four files** in the same build image, keeping the Linux build as it was.
  Windows through `x86_64-pc-windows-gnu` with `gcc-mingw-w64-x86-64`; macOS through
  `cargo-zigbuild` and `ziglang`. The two Python tools are fetched at build time, their versions are
  pinned and their origin is not otherwise verified.
- **The sentence** has one line per connector present, `Connector sha256 ({system}): <digest>`, in
  the order above. `{system}` is the fixed English text, never translated; the label is translated
  in the nine languages with `({system})` before its final colon.
- **The page** no longer names one file. Its steps: find your system with `uname -sm`, a table (what
  `uname` prints, file name, digest line to use), "if your system is not in the table, or the
  message has no digest line for it, do nothing and tell your operator that this hub has no
  connector for your system", one chained command with full paths and no `cd`, and a line for
  macOS (`shasum -a 256 -c -`). The page never holds the invite, the hub key or a digest.
- **The block stays the approved Linux text.** It is emitted only when the Linux connector is
  present; without it the panel hides "Show the command instead", and `invite --command` prints
  the `connector:`/`sha256:` pair of each file present and the bare `join` line, with no chained
  command (what the code does, not a "no connector" sentence).
- **The connector** looks, on Windows, for `<LOCALAPPDATA>\hermes\bin\hermes.exe` when
  `ARAUTO_HERMES_BIN` is unset and `hermes` is not found. Unix is unchanged.

### Relation to decision 0036

The files are served, but this does not make Windows or macOS a supported platform. Decision 0036 (a proposal) is not reopened, and its Windows limits (durable write, named pipe) still hold.

### What is not guaranteed

- **That the Windows or macOS connector works.** The three files were compiled and linked and
  **never run**. The code is exercised on Windows by the suite, but with the MSVC toolchain, not the
  GNU build. There is no Mac available.
- **That Hermes on Windows behaves as read.** Git Bash, the launcher path and `uname` output come
  from documentation and from reading the main branch, not from a run, and not of the installed
  version.
- **That the Git for Windows tools are there.** `curl` and `sha256sum` were not verified on a
  machine.
- **A `.cmd` launcher of Hermes.** The refusal of a batch file stays: such a Hermes needs
  `ARAUTO_HERMES_BIN` set to a real program.
- **The fallback block outside Linux.** It is the Linux text only.
- **The build tools.** `ziglang` and `cargo-zigbuild` are pinned, but their origin is not
  otherwise verified.
