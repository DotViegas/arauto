# 0046 — The key through the entry, and a Hermes without a key

**Status:** proposal · **Date:** 15/11/2026. The reference in the entry was rehearsed and is built. For a Hermes without a key, this file describes the first step (Option A, built); on 15/11/2026 the maintainer asked for Option B, designed and built on 16/11/2026 (below) and **rehearsed once on 16/11/2026, on Hermes Agent v0.23.2 from the official Docker image** (facts N15 to N18 in `referencia/hermes-fatos.md`). It builds on [0023](0023-chave-da-api-local.md) (decided), which it does not reopen.

## Context

The maintainer asked that Arauto configure the key of Hermes's local API by itself when a new Hermes is connected, so that it can be woken. On every real Hermes, waking was off ("Key of the local API of Hermes: not configured"), because the first path of 0023 was never tried. A rehearsal by hand on 15/11/2026, on one Hermes (Hermes Agent v0.22.5, Docker install, Linux), tried it and woke that Hermes ([referencia/hermes-fatos.md](../../referencia/hermes-fatos.md), K1 to K9).

## Decision (proposal)

- **The entry references Hermes's variable.** `join` and `register` save the entry with `--env ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}`. The reference is a fixed constant of the connector, not a secret; Hermes resolves it at spawn. If a version of Hermes rejects `--env` with a non-zero exit, the connector saves the entry without it and the connection goes on.
- **A Hermes without a local API key connects and is told (Option A).** At `join`, `register` and `status` the connector says that this Hermes cannot be woken until its local API is on, naming Hermes's own setting `API_SERVER_KEY` and saying it is set in the configuration of Hermes.
- **Option B: a key only for a Hermes that has none (proposal, built on 16/11/2026, rehearsed once on 16/11/2026, on Hermes Agent v0.23.2 from the official Docker image).** Only in `join`, after the verification code is printed, and only when the entry carries the reference: if Hermes's own command says `API_SERVER_KEY` is not set, nothing accepts a connection on 127.0.0.1:8642 and the configuration names no host for the local API, the connector draws 32 bytes from the system's generator and saves them, as 64 hexadecimal characters, with `hermes config set API_SERVER_KEY <key>`, with no shell and its output discarded. While that command runs, the key sits in its arguments: a departure from [04](../04-seguranca-conexao.md) and from [0023](0023-chave-da-api-local.md) that needs the maintainer's approval. The connector never writes Hermes's secrets file itself and never restarts or signals Hermes: the operator sends `/restart` after the approval. `register`, `status` and `leave` make no key, and `leave` does not remove it. On Docker the image makes a key at first boot (rehearsal 16/11/2026); whether the step then stays idle there is not proved yet.
- **`arauto-connector register`** saves the entry again for a Hermes connected before this change. It asks the hub and the leader nothing and touches neither the identity nor the credential.
- **`local_waking`.** `arauto_status` gains a field added by the connector: whether the connector of this Hermes received the key. Its description does not change. The hub does not change.
- **The connector reads its own variables only,** and the key only in the `mcp` process. The `hermes` command it runs does not receive `API_SERVER_KEY` or `ARAUTO_HERMES_API_KEY` in its environment; the one `config set` of Option B receives the key it saves as an argument.

## Not guaranteed

- One Hermes, one version, one system were observed. Windows, macOS, v0.23 and other versions are unproved, as is the shim `/opt/hermes/bin/hermes` handling an argument with `${...}`.
- A running gateway applies a changed entry only after it restarts (K5); the connector does not restart it. A copy started from a new entry while an old leader lives does not wake.
- **New exposure:** the key sits in the initial environment of each connector copy; on Linux any process of the same user, the LLM's terminal included, can read it from `/proc/PID/environ` without the approval Hermes asks before reading its secrets file. Hermes already does the same for the provider keys it injects (K6). Whether this is acceptable under principle 2, and whether the threat model ([05](../05-modelo-de-ameacas.md)) gets a row, is for the maintainer.
- A Hermes without a key keeps bouncing its notices (`failed`, back to pending with waits up to 5 minutes) until they expire. The hub does not change.
- The explanation of K7 ("not configured" from `status`) is not established; the code reads the key from the leader.

## Open points for the maintainer

- Option B: approve the key in the arguments of `config set`, the texts and the rehearsal outside Docker. Where no key is made, Option A is what the connector does.
- Confirm the field `local_waking` and its name.
- Decide what to do about K5 ([facts](../../referencia/hermes-fatos.md)): a `join` that installs a new program asks the leader to end, but a running gateway did not start a dead server again in 15 minutes.

## Alternatives considered

- **Parts of Option B not taken:** the connector writing Hermes's secrets file itself (it would read every secret there), restarting or signalling Hermes, and making a key from `register`, which a hostile note could lead the Hermes to run.
- **Reading the key from the secrets file** (second path of 0023): not needed.
