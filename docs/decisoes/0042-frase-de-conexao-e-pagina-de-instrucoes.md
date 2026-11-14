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
