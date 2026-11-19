# 0046 — The key through the entry, and a Hermes without a key

**Status:** proposal · **Date:** 15/11/2026. The reference in the entry was rehearsed and is built. For a Hermes without a key, this file describes the first step (Option A, built); on 15/11/2026 the maintainer asked for Option B, designed and built on 16/11/2026 (below) and **rehearsed once on 16/11/2026, on Hermes Agent v0.23.2 from the official Docker image** (facts N15 to N18 in `referencia/hermes-fatos.md`). On 18/11/2026 the tools of a woken run (section below, proposal, built, not tried on a real Hermes). It builds on [0023](0023-chave-da-api-local.md) (decided), which it does not reopen.

## Context

The maintainer asked that Arauto configure the key of Hermes's local API by itself when a new Hermes is connected, so that it can be woken. On every real Hermes, waking was off ("Key of the local API of Hermes: not configured"), because the first path of 0023 was never tried. A rehearsal by hand on 15/11/2026, on one Hermes (Hermes Agent v0.22.5, Docker install, Linux), tried it and woke that Hermes ([referencia/hermes-fatos.md](../../referencia/hermes-fatos.md), K1 to K9).

## Decision (proposal)

- **The entry references Hermes's variable.** `join` and `register` save the entry with `--env ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}`. The reference is a fixed constant of the connector, not a secret; Hermes resolves it at spawn. If a version of Hermes rejects `--env` with a non-zero exit, the connector saves the entry without it and the connection goes on.
- **A Hermes without a local API key connects and is told (Option A).** At `join`, `register` and `status` the connector says that this Hermes cannot be woken until its local API is on, naming Hermes's own setting `API_SERVER_KEY` and saying it is set in the configuration of Hermes.
- **Option B: a key only for a Hermes that has none (proposal, built on 16/11/2026, rehearsed once on 16/11/2026, on Hermes Agent v0.23.2 from the official Docker image).** Only in `join`, after the verification code is printed, and only when the entry carries the reference: if Hermes's own command says `API_SERVER_KEY` is not set, nothing accepts a connection on 127.0.0.1:8642 and the configuration names no host for the local API, the connector draws 32 bytes from the system's generator and saves them, as 64 hexadecimal characters, with `hermes config set API_SERVER_KEY <key>`, with no shell and its output discarded. While that command runs, the key sits in its arguments: a departure from [04](../04-seguranca-conexao.md) and from [0023](0023-chave-da-api-local.md) that needs the maintainer's approval. The connector never writes Hermes's secrets file itself and never restarts or signals Hermes: the operator sends `/restart` after the approval. `register`, `status` and `leave` make no key, and `leave` does not remove it. On Docker the image makes a key at first boot (rehearsal 16/11/2026); whether the step then stays idle there is not proved yet.
- **`arauto-connector register`** saves the entry again for a Hermes connected before this change. It asks the hub and the leader nothing and touches neither the identity nor the credential.
- **`local_waking`.** `arauto_status` gains a field added by the connector: whether the connector of this Hermes received the key. Its description does not change. The hub does not change.
- **The connector reads its own variables only,** and the key only in the `mcp` process. The `hermes` command it runs does not receive `API_SERVER_KEY` or `ARAUTO_HERMES_API_KEY` in its environment; the one `config set` of Option B receives the key it saves as an argument.

## The tools of a woken run (proposal, 18/11/2026)

**Context.** One Hermes v0.23.0 had an explicit list `platform_toolsets.api_server` without `arauto`: every woken run was answered with text and never read the notes, while `arauto_status` said `local_waking: true` (`referencia/hermes-fatos.md`, P1 to P5).

**Decision, approved by the maintainer on 18/11/2026.** T1: at `join`, the connector reads that list through Hermes's own command (`config get`) and, when it is an explicit non-empty list without `arauto`, appends `arauto` with `config set`, keeping every other entry, reads it back, and says so in the output. A missing row, a list that already includes it and an empty list change nothing. T2: `status` and `arauto_status` say whether the list of tools of woken runs includes `arauto`, and what to do when it does not, with fixed text in the connector. Built as in [11](../11-conector.md), section 6: only in `join`, only when the entry carries the reference, after the key step; no rollback; no restart; a record file `woken-runs.json`; two new fields of `arauto_status` (`woken_run_tools`, `woken_run_tools_text`); `local_waking` unchanged; the leader runs no command of Hermes; `status` reads the list live and writes nothing.

**Not guaranteed.** How `config get` prints a list (one `  - NAME` line per item, `[]` when empty; a one-line `[a, b]` is not read as a list, because a text value prints the same way) and how `config set` stores one (a real list) were observed on Hermes Agent v0.23.0 and v0.22.5 on 18/11/2026 (N25), so another form, and a run of items after a line that could belong to the value (indented, an item, a comment, a key), is "unreadable" and gets no write; that a woken run calls the Arauto tools once `arauto` is in the list was seen once, after a gateway restart (P4, 19/11/2026); the meaning of an empty list for a woken run is unknown; whether a running gateway needs a restart was not tried; a toolset name other than the key of the server is not recognized; `arauto_status` shows what `join` found, not the current list; the change may need a gateway restart; `leave` does not take `arauto` out of the list.

**Open points.** The part of P6 that N25 did not settle: whether a gateway restart is needed for the new list, and what an empty list means for a woken run. [04](../04-seguranca-conexao.md), C6, now names the appended `arauto` as the second lasting change, beside the key (wording approved by the maintainer on 18/11/2026).

## Not guaranteed

- One Hermes, one version, one system were observed. Windows, macOS, v0.23 and other versions are unproved, as is the shim `/opt/hermes/bin/hermes` handling an argument with `${...}`.
- A running gateway applies a changed entry only after it restarts (K5); the connector does not restart it. A copy started from a new entry while an old leader lives does not wake.
- **New exposure:** the key sits in the initial environment of each connector copy; on Linux any process of the same user, the LLM's terminal included, can read it from `/proc/PID/environ` without the approval Hermes asks before reading its secrets file. Hermes already does the same for the provider keys it injects (K6). Whether this is acceptable under principle 2, and whether the threat model ([05](../05-modelo-de-ameacas.md)) gets a row, is for the maintainer.
- A Hermes without a key keeps bouncing its notices (`failed`, back to pending with waits up to 5 minutes) until they expire. The hub does not change.
- The explanation of K7 ("not configured" from `status`) is not established; the code reads the key from the leader.

## Open points for the maintainer

- Option B: approve the key in the arguments of `config set`, the texts and the rehearsal outside Docker. Where no key is made, Option A is what the connector does.
- Confirm the field `local_waking` and its name.
- Decide what to do about K5 ([facts](../../referencia/hermes-fatos.md)): a `join` that installs a new program asks the leader to end, but a running gateway did not start a dead server again in 15 minutes. Superseded in part by decision 0053 (proposal, 19/11/2026): `join` no longer asks the leader to end while the folder format is the same; the running program keeps serving until the gateway restarts.

## Alternatives considered

- **Parts of Option B not taken:** the connector writing Hermes's secrets file itself (it would read every secret there), restarting or signalling Hermes, and making a key from `register`, which a hostile note could lead the Hermes to run.
- **Reading the key from the secrets file** (second path of 0023): not needed.
