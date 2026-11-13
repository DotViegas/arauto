# Security

> **Status: first MVP under construction.** There is no version for use yet. This file describes what the design sets out to
> guarantee, what it does not guarantee and how to report a flaw.

## How to report a flaw

**Do not open a public issue for a security flaw.**

The channel will be GitHub's private vulnerability reporting, in the security tab of the repository
([decision 0027](docs/decisoes/0027-canal-de-relato-de-falha.md)). It comes into existence when the
repository is opened to the public. Until then, the project has no version in use by third parties.

When reporting, include: what you achieved, the steps to repeat it, the version of the hub and of
the connector and what an attacker would gain.

## What the design sets out to guarantee

These are goals. They only become guarantees after the tests foreseen in the
[roadmap](docs/12-roteiro.md). They hold while the hub, the admin's account and the system's random
generator are intact.

1. The chat in which you pasted the connection block, if it leaks **after** the use, contains
   nothing that connects.
2. If it leaks **before** the use, whoever read it does not get access without an authenticated
   admin typing their code.
3. No long-lived secret passes through the chat, appears on a screen or enters a log.
4. Whoever takes a copy of the files of a Hermes outside and uses it is discovered when the real
   device presents its credential again: in seconds, if it is on, in a version the hub accepts, and
   reaches the hub (in the worst case, 90 seconds); on its return, if it was off or with its network
   held by whoever copied. The hub alerts and the device stays stopped.
5. A Hermes only reaches the numbers that were given to it.
6. Revoking a Hermes or removing an access holds on the next call.
7. Whoever only reads the hub's database cannot pass for a Hermes.

The detail is in [docs/04-seguranca-conexao.md](docs/04-seguranca-conexao.md).

## What the project does not guarantee

We say this beforehand so that nobody finds out afterwards.

1. **Hermes's LLM reaches the files of the connector** when it is Hermes itself that installs it. It
   cannot open the credential, but the file is within its reach, to read and to write. A process
   that stays on the machine, with the same user, can use a copy outside without leaving a sign.
   Whoever needs it not to reach them uses the shielded mode, which does not exist yet.
2. **A copy of the credential, used with the real Hermes off, works until it comes back.** There is
   no time limit. If it never comes back, the copy is never discovered. The same holds while whoever
   copied holds the network of the real Hermes. And a Hermes with the connector below the hub's
   minimum version counts as off: the copy works until the admin reconnects it, and its use may not
   show up as a copy. It is decided that the hub will check the credential of that connector before
   refusing it ([decision 0030](docs/decisoes/0030-conector-desatualizado.md)); while this is not
   proved, this limit holds.
3. **Whoever has an old copy of the files of the connector, or only the identity, can take Hermes
   down.** They gain no access, but cause the quarantine or the closing.
4. **A manipulated LLM uses the tools Hermes already has.** If a Hermes may send on a number, a
   malicious instruction it obeys can send through it. The project limits the reach and records
   everything; it does not prevent it.
5. **An admin account taken over or a hub server broken into brings everything down.**
6. **Whoever has the volume of the engine sessions has the WhatsApp.** The engine needs to stay
   without a public port.
7. **Restoring the hub from an old backup undoes** what was revoked, swapped or removed after it.
   The hub suspends the active devices and warns; each Hermes needs to be reconnected. If the whole
   server comes back from an image, the hub only notices when a device that swapped its credential,
   or was connected, after it connects; until then, what was revoked holds again. In any
   restoration, an admin removed after the backup has access again until removed again.
8. **In every installation through the block, the digest of the connector comes from the panel.** A
   hub that was broken into points to a fake program. Check it against the digest published in the
   project's versions.
9. **WhatsApp may block the number.** The engine is an unofficial client.

The project will never say that it "closes every gap".

## Roots of trust

Three things, if compromised, give access to everything in an installation. Protect them in this
order:

| Root | How to protect |
|---|---|
| The admin's account | Passkey. The panel comes closed, on a private network, by default; always with TLS. |
| The hub's server | Updates up to date, restricted access, master key outside the database and outside the ordinary backup. |
| The WhatsApp engine | Only on the internal network, with a key of its own for the installation. |

## Versions of the connector

The design foresees that every published version of the connector is signed with the project's
release key, which stays in no hub, and that the SHA-256 digest of each version stays published in
the repository. The connection block carries that digest: check it against the published one.
