# 0046 — The key through the entry, and a Hermes without a key

**Status:** proposal · **Date:** 15/11/2026. The reference in the entry was rehearsed and is built. For a Hermes without a key, this file describes the first step (Option A, built); on 15/11/2026 the maintainer asked for Option B, which is **not designed and not built yet**. It builds on [0023](0023-chave-da-api-local.md) (decided), which it does not reopen.

## Context

The maintainer asked that Arauto configure the key of Hermes's local API by itself when a new Hermes is connected, so that it can be woken. On every real Hermes, waking was off ("Key of the local API of Hermes: not configured"), because the first path of 0023 was never tried. A rehearsal by hand on 15/11/2026, on one Hermes (Hermes Agent v0.22.5, Docker install, Linux), tried it and woke that Hermes ([referencia/hermes-fatos.md](../../referencia/hermes-fatos.md), K1 to K9).

## Decision (proposal)

- **The entry references Hermes's variable.** `join` and `register` save the entry with `--env ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}`. The reference is a fixed constant of the connector, not a secret; Hermes resolves it at spawn. If a version of Hermes rejects `--env` with a non-zero exit, the connector saves the entry without it and the connection goes on.
- **A Hermes without a local API key connects and is told (Option A).** At `join`, `register` and `status` the connector says that this Hermes cannot be woken until its local API is on, naming Hermes's own setting `API_SERVER_KEY` and saying it is set in the configuration of Hermes.
- **The connector never writes into Hermes's secrets file, never creates a key for Hermes and never restarts Hermes.** Option B, which would do those things, was asked for by the maintainer on 15/11/2026 and is not built: it needs its own design, a rehearsal on a real Hermes and its own decision, because it changes what the connector may touch.
- **`arauto-connector register`** saves the entry again for a Hermes connected before this change. It asks the hub and the leader nothing and touches neither the identity nor the credential.
- **`local_waking`.** `arauto_status` gains a field added by the connector: whether the connector of this Hermes received the key. Its description does not change. The hub does not change.
- **The connector reads its own variables only,** and the key only in the `mcp` process. The `hermes` command it runs does not receive `API_SERVER_KEY` or `ARAUTO_HERMES_API_KEY`.

## Not guaranteed

- One Hermes, one version, one system were observed. Windows, macOS, v0.23 and other versions are unproved, as is the shim `/opt/hermes/bin/hermes` handling an argument with `${...}`.
- A running gateway applies a changed entry only after it restarts (K5); the connector does not restart it. A copy started from a new entry while an old leader lives does not wake.
- **New exposure:** the key sits in the initial environment of each connector copy; on Linux any process of the same user, the LLM's terminal included, can read it from `/proc/PID/environ` without the approval Hermes asks before reading its secrets file. Hermes already does the same for the provider keys it injects (K6). Whether this is acceptable under principle 2, and whether the threat model ([05](../05-modelo-de-ameacas.md)) gets a row, is for the maintainer.
- A Hermes without a key keeps bouncing its notices (`failed`, back to pending with waits up to 5 minutes) until they expire. The hub does not change.
- The explanation of K7 ("not configured" from `status`) is not established; the code reads the key from the leader.

## Open points for the maintainer

- Option B (asked for on 15/11/2026): design it, rehearse it and record it in a decision of its own. Until then Option A is what the connector does.
- Confirm the field `local_waking` and its name.
- Decide what to do about K5 ([facts](../../referencia/hermes-fatos.md)): a `join` that installs a new program asks the leader to end, but a running gateway did not start a dead server again in 15 minutes.
- Optional wording of the instruction page, not changed: ask the Hermes to send, with the verification code, the line the connector printed about waking.

## Alternatives considered

- **Option B** (the connector reads or writes Hermes's secrets file, creates a key, or restarts Hermes): not built in this step; asked for by the maintainer on 15/11/2026 and left for a decision of its own, since it crosses the line of what the connector may touch today.
- **Reading the key from the secrets file** (second path of 0023): not needed.
