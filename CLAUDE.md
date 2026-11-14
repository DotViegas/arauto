# CLAUDE.md

Guidance for development sessions in this repository.

## The project

Arauto gives each Hermes agent controlled access to WhatsApp numbers and lets one Hermes send a note
to another: a **hub** (server) and a **connector** (a program beside Hermes). It is free, and the
code will be open, under the Apache-2.0 license. In the brand, the name is written Arauto[IA]
(`docs/decisoes/0026-nome-do-projeto.md`).

**Status: first MVP under construction** (`docs/15-mvp.md`). Read in this order: `README.md`,
`docs/glossario.md`, `docs/01-visao.md`, `docs/04-seguranca-conexao.md`,
`docs/14-protocolo-do-canal.md` and `docs/15-mvp.md`.

## Working rules

### What is public and what is internal

- Everything that is versioned here is **public**. Do not write the name of a person, of a company,
  of a server, an internal address, a phone number or any real data. In examples, use addresses and
  numbers reserved for documentation.
- Notes of the maintainer of the project stay in the `interno/` folder, which is in `.gitignore`.
  Never use `git add -f` on it and never copy excerpts from it into a public document.
- Before any commit, search for real data in what is going in.

### Vocabulary

- Use the terms of `docs/glossario.md` and respect the "words we do not use" list.
- **Invite** is the temporary one, which dies on first use. **Sealed credential** is what Hermes
  keeps to come back. Call neither of them "ticket" or "token".
- **Connect** is for Hermes. **Pair** is for numbers.
- "Session" always comes with its owner: channel session, engine session, panel session.
- The project is free: do not write about price, sale or subscription.

### Fact, proposal and pending item

Every document separates:

- **fact**: checked, with version and date;
- **proposal**: a design not proved yet;
- **pending decision**: what still needs to be chosen.

Do not promote a proposal to fact without a test. Do not write in the present tense, as if it
existed, what is still design. When checking something on Hermes or on the engine, record it in
`referencia/`, with the version and the date.

### Decisions

- Each design decision has a file in `docs/decisoes/`, with the status: decided, proposal or
  pending.
- A **decided** decision is not reopened without a request from the maintainer of the project.
- The design changed: update the affected document **and** the decision.

### Needs the maintainer's approval

- The text of the connection block (`docs/02-como-funciona.md`).
- Any change in `docs/04-seguranca-conexao.md`.
- License, opening of the repository and any published version.

## Security principles

They hold for all code and every document.

1. **No invented cryptography.** Compose known pieces and say which ones.
2. **A long-lived secret never passes through a person:** it does not appear on a screen, in a chat,
   log, audit or tool response. A short-lived secret (invite, pairing code, the installation code
   issued by the command) appears once, where it is needed, and is never kept in the clear. One
   exception, and only this one: the installation code of a hub where no passkey was ever
   registered has no deadline. It serves once: the first passkey spends it and each start of the
   hub replaces it. Until then the hub writes it again in the output of its start, from the memory
   of the process alone, and the database keeps only its digest.
3. **What is only checked is kept as a public key or a digest.** What needs to be read back stays
   encrypted, with the key outside the database.
4. **Random draws only from the system's generator.** Nothing derived from date, time, name or
   counter.
5. **A read request does not change state.** Opening an address never spends an invite.
6. **The hub does not send free text to the LLM.** A notice is a pointer; the fixed sentence and the
   descriptions of the tools stay in the connector. The hub may serve a fixed page compiled into it, whose only
   fields have a strict format (host, Hermes name, UTC time, version, file name).
7. **The connector does not execute a command from the hub.** The only thing it discards on
   information from the hub is a connection request that has not received a credential yet. The
   folder of an old device goes away by decision of the connector, after another one takes over and
   after the hub answers.
8. **The hub only isolates.** No business rule: no content filter, no automatic reply, no choosing
   which Hermes acts.
9. **Authorization on every call,** in the hub, against the current state.
10. **Every text coming from a device or from WhatsApp is hostile:** validate on input, escape on
    output.
11. **The channel session only serves after the swap of the credential is confirmed.** After the
    first confirmed one, never issue another only on the identity.
12. **Say what is not guaranteed.** Never write that the project "closes every gap".

## In the code

- Rust in both programs. A workspace with five parts (`docs/15-mvp.md`, section 4): the common
  protocol, what changes from one system to another, the hub, the connector and the test material.
- No `unsafe`. Where the protocol (`docs/14-protocolo-do-canal.md`) and
  `docs/04-seguranca-conexao.md` diverge, 04 prevails.
- Development runs directly through `cargo`, with a local PostgreSQL. A container is only for the
  delivery in production.
- English is the main language of the project, in everything that is versioned: names in the
  code, descriptions of the tools, error messages, the text of the screens, commit messages and
  documents (`docs/decisoes/0016-idioma.md`).
- `Cargo.lock` versioned; compile with `--locked`.
- Run, before saying that something is ready: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `cargo test --workspace --locked`, with `ARAUTO_TEST_DATABASE_URL` pointing to a test PostgreSQL.
  Report: what ran, what passed, what failed and what was left without coverage. What was only read
  does not count as tested.
- A new security rule comes with a test that fails when the rule is removed.
- Queues and the swap of the credential have a test with randomized rounds: drops, delays and
  repetitions.
- The automatic tests run against a fake engine and a fake Hermes. No automatic test on a real
  WhatsApp number. The exception is the contract test, done by hand, with a test number.

## What never to do

- Log message content or a secret.
- Expose the WhatsApp engine outside the internal network.
- Embed the engine in the delivery of the project.
- Create an alternative connection path "to make it easier" that skips the invite or the
  verification code.
- Open a network port on Hermes's machine.
