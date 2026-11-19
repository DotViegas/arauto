# Facts checked on Hermes

**Status: checked facts**, with the origin of each statement. What the design of Arauto assumes
about Hermes Agent.

**Checked on 09/11/2026, on Hermes Agent v0.22.5 (2026.9.24), in a Linux x86_64 container.** Version
0.23.0 is also a target and has not been checked point by point yet.

Marking: **[code]** read in the installed code; **[observed]** seen in a running Hermes;
**[to check]** an assumption with no test yet.

## Tool servers (MCP)

| Fact | Origin |
|---|---|
| The MCP client speaks stdio, HTTP and SSE. There is no websocket as a transport. | [code] `tools/mcp_tool.py` |
| There is OAuth for MCP servers, including the device flow (RFC 8628), started only by `hermes mcp login`. | [code] `tools/mcp_oauth_device.py` |
| Official command: `hermes mcp add NAME --command PROGRAM --args ...` or `--url ADDRESS --auth oauth\|header`. Also `remove`, `list`, `test`. | [observed] help of the command; it is **not** non-interactive: see "Registering a stdio server" below. |
| The list of servers is reconciled by itself, in less than a minute, without restarting, for a **new name**. A changed entry, or a server whose process died, is not applied by a running gateway until it restarts (K5). | [observed] 09/11/2026; [observed] 15/11/2026 |
| An MCP server appears on all of Hermes's conversation platforms, unless configured per platform. | [observed] |
| Every server entry is checked on saving and on starting. One that uses a command interpreter with a network call or with a sign of data being sent out is refused. | [code] `hermes_cli/mcp_security.py` |
| The stdio server receives a **filtered** environment: the user's folder, the program path, temporary folders and the variables of the entry itself. It does not inherit Hermes's environment. Since 15/11/2026: a value in the entry's `env` of the form `${VAR}` is resolved from Hermes's own secret scope (K3), and the variables that Hermes's secret source injects for every stdio child are also present (K6). | [code] `_build_safe_env`; [observed] on a server in use |
| On shutting down, Hermes closes the server's input, waits 2 seconds, asks for termination and only then forces it. A supervisor shuts the servers down if Hermes dies. | [code] MCP SDK and `tools/mcp_death_supervisor.py` |
| The stdio server lives as long as the Hermes process that started it lives. | [observed] 10 days without a restart |
| There is handling for the notice that the list of tools changed. | [code] `tools/mcp_tool_health.py` |

## The LLM's terminal

| Fact | Origin |
|---|---|
| The terminal runs on the machine itself, with the **same user** as Hermes. | [observed] configuration and processes |
| The user's folder is Hermes's data volume, with read, write and execute. | [observed] |
| Each command has a limit of 180 seconds. | [observed] configuration |
| Before running, Hermes asks for human approval for: content from the internet sent straight to the interpreter (`curl ... \| sh`), access to Hermes's secrets file, known credential files and variables with a secret's name. | [code] `tools/approval_detection.py` |
| The output of the tools has secrets masked, by format pattern and by variable name. | [code] `agent/redact.py` |
| `curl`, `sha256sum`, `flock` and `tar` exist in the image. | [observed] |
| A process does not read the memory of another that is not its descendant. | [observed] on the server checked; depends on the system |

Consequence for the project: everything the connector writes in the user's folder, the LLM can read.

## Processes

| Fact | Origin |
|---|---|
| The services of the container are supervised with administrator privilege. Hermes's user does not register a new service that survives a restart. | [observed] |
| On a Hermes with one stdio server configured there was **one** copy of it, a child of the messaging process. | [observed] |

## Local API

| Fact | Origin |
|---|---|
| `POST /v1/runs` starts a run. It requires a key of its own. By default it listens only on the local address. | [observed] |
| The API key does not stay in the environment of the process: Hermes reads it from the secrets file. | [observed] variable names only |
| There is support for a repetition identifier in the API. | [code] `gateway/platforms/api_server.py` |
| The context of each conversation tells the origin: the platform and with whom. | [code] `gateway/session.py` |

## Native WhatsApp channel

| Fact | Origin |
|---|---|
| A direct conversation accepts the policies pairing, list, open and off. Only the list policy restricts to whoever is on the list. | [code] `gateway/platforms/access_policy_mixin.py` |
| A group is only forwarded to Hermes with the list or the open policy. | [code] same |

## To check

The single list of what needs to be proved is in [docs/12-roteiro.md](../docs/12-roteiro.md), Phase 1.
These are the assumptions about Hermes:

| Assumption | Why it matters |
|---|---|
| The registration of the connector through the official command passes the check of entries. Not proved: the trial of 14/11/2026 (below) registered an entry, but **interactively**, and the check of entries is still to be seen. | Without this there is no quick mode. |
| How many Hermes processes start the server when panel and scheduled routines are active. | It defines the work of the leader. |
| ~~The server entry accepts a reference to a Hermes variable in the environment field.~~ Observed on 15/11/2026, one Hermes (K2, K3). Not checked on Windows, macOS or other versions. | It is the cleanest path to the key of the local API. |
| Whether the block triggers the terminal's approval request or not. | The flow works in both cases; the screen needs to say what to expect. |
| Hermes keeps procedures in memory or as a skill from what it executes. | The block asks it not to keep them. |
| ~~Address, body and repetition identifier of the call to the local API.~~ A wake by the fixed sentence was accepted and answered on 15/11/2026, one Hermes (K4). The use of the repetition identifier was not separately checked. | It is through it that Hermes is woken. |
| Hermes restarts a tool server that exits. Observed on 15/11/2026: a running gateway did **not** restart a killed connector in 15 minutes (K5). | Since decision 0053 (19/11/2026) a reconnect no longer ends the running connector while the folder format is the same; the new file runs at the next gateway restart. It still matters when the format differs. |
| The behavior of version 0.23.0 on each line of this file. | 0.23 opens more tools by default. |
| What an empty `platform_toolsets.api_server` means and whether a running gateway applies a changed list (P6, below; how `config get` and `config set` handle the list is N25). | The connector appends `arauto` to that list at `join` ([decision 0046](../docs/decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md)). |

## First real connection attempt (14/11/2026)

**Fact, [observed], 14/11/2026.** The first real attempt to connect a production Hermes with the
connection block alone (the text of [02](../docs/02-como-funciona.md), section 1, now the
fallback) was **refused by that Hermes**. The cause is **unknown**: the Hermes version was not
recorded here.

**Not known:** which part of the block it refused (the command, the form of the message, the
channel it came from, or something else); and whether it would also refuse the connection
sentence and the instruction page of
[decision 0042](../docs/decisoes/0042-frase-de-conexao-e-pagina-de-instrucoes.md). That design is a
proposal **not tried on a real Hermes yet**. When it is, record here the version, the date and what
the Hermes said.

**Fact, [observed], 14/11/2026, Hermes Agent v0.22.5 (Docker install).** The same production
Hermes received the connection sentence in Portuguese, as a new message, and did **not** refuse:
it read the instruction page, downloaded the connector, checked the digest and ran `join`. The
connection then stopped at the registration (AR-601), for the reasons in the next section, and
the Hermes did not try another way, as the page tells it. One attempt, one Hermes: it shows the
sentence can be accepted, not that it always is. The connection was not completed in that
attempt.

## Registering a stdio server (checked 14/11/2026)

**Checked on 14/11/2026, on Hermes Agent v0.22.5, in a throwaway container with an empty data
folder. [observed]**, except where marked. Another version may differ.

| # | Fact |
|---|---|
| F1 | `hermes mcp add arauto --command <absolute path> --args mcp` starts the server, lists its tools, then asks on the terminal "Enable all N tools? [Y/n/select]". With the standard input at end of file it prints "Cancelled.", saves **nothing** and exits 0. |
| F2 | With `y` and a newline on the standard input it saves the entry, enabled, and exits 0. |
| F3 | When an entry of that name already exists it first asks "Server 'arauto' already exists. Overwrite? [y/N]" (end of file = cancelled, exit 0). With `y`, newline, `y`, newline it overwrites and saves it enabled. |
| F4 | When the probe of the server fails it asks "Save config anyway (you can test later)? [y/N]"; with `y` it saves the entry **disabled**, exit 0. |
| F5 | The exit code is 0 in every one of those cases: it says nothing about whether the entry was saved. |
| F6 | `hermes mcp list` prints a table. The row of a server starts, after spaces, with its name and holds "✓ enabled" or the word "disabled". Example row: `  arauto           /opt/data/.arauto/arauto-...   all          ✓ enabled`. With no server the name does not appear. |
| F7 | `hermes mcp remove arauto` with the standard input at end of file removes the entry (the default answer is yes), exit 0. When the entry does not exist it prints "not found", exit 0. |
| F8 | In the Docker install the program is `/opt/hermes/bin/hermes` (a small shim) and `/opt/hermes/.venv/bin/hermes`. The `PATH` of the container has both, but the terminal in which the agent runs commands did **not** have `hermes` on its `PATH` (observed on a production Hermes). The install script for Linux and macOS puts the launcher in `~/.local/bin` ([read] in its documentation, **not run**). |

Consequence for the connector (`docs/11-conector.md`, section 4): the registration needs
answers on the standard input and a check in the list, and the program of Hermes may have to be
looked for in fixed places. The line above, that the stdio registration passes by itself, was
never proved and is not a fact.

**Checked on 14/11/2026, same version, same kind of throwaway container.** A whole `join` with
the connector built from the working tree, as the user of Hermes, with `hermes` taken out of the
`PATH` and an empty data folder: the connector found the program in the fixed places, the entry
was saved enabled (`command` the fixed path of the connector, `args` only `mcp`), `hermes mcp
list` showed it and the verification code came out, in about 10 seconds. The list read by a
program came without color codes.

| | Fact |
|---|---|
| F9 | The shim `/opt/hermes/bin/hermes`, run by the administrator of the system, changes to the user of Hermes before it runs. A connector kept by the administrator in a private folder is then not executable by the probe ("Permission denied"), and the entry is saved disabled. Run as the user of Hermes, as the agent's terminal does, the same command works. |

**Checked on 14/11/2026, same version, by running the terminal tool of Hermes itself
(`tools/terminal_tool.py`) and reading its code in the image.**

| | Fact |
|---|---|
| F10 | In a container, when the folder `home` exists inside the data folder, the terminal of the agent runs with `HOME` at that folder (`/opt/data/home`), while the server of tools that Hermes starts keeps the `HOME` of the Hermes process (`/opt/data`). [code] `hermes_constants.get_subprocess_home`, `tools/mcp_tool_config._build_safe_env`; [observed]. A connector that looks for its folder by `HOME` finds one folder in `join` and another when Hermes starts it: after the approval it stays without a hub. With the folder in the entry (`--home`), the same trial reached "active" on the hub and `arauto_status` answered connected. |
| F11 | The terminal tool remembers the folder where the last command of the session ended and, before each next command, runs `cd` to it; when that folder no longer exists, **every** command of the session fails before it starts, with `cd: ...: No such file or directory` and exit 126, until a command is given an explicit working folder. [code] `tools/environments/base_session_env.py`; [observed], with the same message a production Hermes showed. A page whose command ends inside `~/.arauto` leaves the session there. |
| F12 | The `PATH` of the terminal tool was `/usr/local/bin:/usr/bin:/bin:/usr/local/games:/usr/games`: without the folders of Hermes. [observed] |
| F13 | The command of the instruction page (full paths, no `cd`) passed through the terminal tool without any approval question, and the whole of it took about 10 seconds. [observed] |

Not checked: a native install (outside a container); whether a running Hermes picks the new
entry up by itself or needs a new session; the tools of WhatsApp after an access is granted.

## Windows and macOS (14/11/2026)

Stated by the maintainer's assistant on 14/11/2026. **None of this was run on a real Windows
Hermes or on a Mac. Do not read any row as proved.** The version read is the main branch, not the
installed one. Used by [decision 0042](../docs/decisoes/0042-frase-de-conexao-e-pagina-de-instrucoes.md)
and [11-conector.md](../docs/11-conector.md).

| | Fact |
|---|---|
| W1 | [observed] In the build image (`rust:1.98-bookworm`) the connector, with its `tls` feature, compiles and links for `x86_64-pc-windows-gnu` (after `gcc-mingw-w64-x86-64` and `rustup target add`; `arauto-connector.exe`, about 13 MB) and for `aarch64-apple-darwin` and `x86_64-apple-darwin` (with `ziglang` 0.17.1 and `cargo-zigbuild` 0.24.1 from `pip`; about 7.5 MB each). The linker warns that no Apple SDK was found and still links. The arm64 file carries an ad hoc code signature command; the x86_64 one does not need one. |
| W2 | [observed] None of the Windows or macOS files was ever executed. The code of the connector runs on Windows in the test suite, but with the MSVC toolchain, not this GNU build. |
| W3a | [code read, main branch, not run] On native Windows the terminal of the Hermes agent runs commands through Git Bash, so POSIX commands (`mkdir -p`, `curl`, `chmod`) are the ones to give it. |
| W3b | [documentation] `sha256sum` and `curl` come with Git for Windows. Not verified on a machine. |
| W3c | [code read, main branch, not run] The launcher is a real `hermes.exe` in `<HERMES_HOME>\bin`, and `HERMES_HOME` defaults to `%LOCALAPPDATA%\hermes`; it falls back to a `.cmd` only when its tool to make launchers is missing. |
| W3d | [code read, main branch, not run] Outside a container the terminal keeps the real `HOME`. |
| W3e | [documentation] In Git Bash, `uname -s` starts with `MINGW64_NT` or `MSYS_NT`. |
| W4a | [documentation] macOS has no `sha256sum` by default; it has `shasum -a 256`. |
| W4b | [documentation] `uname -sm` prints `Darwin arm64` or `Darwin x86_64`. |
| W4c | [documentation] The Hermes install script puts the launcher in `~/.local/bin` (read, not run). |

Not checked: a Windows or macOS Hermes running the page's command; whether the Windows or macOS
connector starts, registers itself in Hermes or holds a channel.

**Checked on 14/11/2026, on a Windows 11 machine with Git for Windows and no Hermes, with the
Windows file taken from the image built from the working tree.**

| | Fact |
|---|---|
| W5 | [observed] The whole image builds with the four files in its downloads folder. |
| W6 | [observed] In Git Bash, `uname -sm` printed `MINGW64_NT-10.0-26300 x86_64`, and `curl` and `sha256sum` were on the `PATH`. |
| W7 | [observed] The steps of the page, run in Git Bash with the Windows file: the digest checked, the file ran, `join` reached the hub over `wss://` and left a request waiting for the code. With no Hermes on the machine it ended in AR-601 with the sentence that the program of Hermes was not found, and installed itself as `arauto-connector.exe`. |

Still not checked: the same steps inside the terminal of a Hermes on Windows, the registration in
a Hermes on Windows, and anything at all on a Mac.

**Checked on 14/11/2026, on a Windows 11 machine with Hermes Agent v0.22.2 installed natively,
by running the terminal tool of that Hermes with its own Python, in a separate data folder and a
separate user folder (the configuration in use was not touched).**

| | Fact |
|---|---|
| W8 | [observed] The terminal of the agent is Git Bash: `uname -sm` printed `MSYS_NT-10.0-26300 x86_64`; `curl` (the one of Git, a native Windows program) and `sha256sum` were on the `PATH`, and so was `hermes`; `MSYS_NO_PATHCONV` was `1`. |
| W9 | [observed] With that setting, `curl -o` given a path in the form `/c/Users/...` fails with `curl: (23) client returned ERROR on write`: the program does not understand the path and Git Bash does not convert it. This stopped a real connection attempt. With the download written by the shell (`curl ... > "$D/$F"`) it works. |
| W10 | [observed] The launcher is `hermes.exe` in `%LOCALAPPDATA%\hermes\bin`, beside a `hermes.cmd`. |
| W11 | [observed] The registration with answers on the input and the output discarded saves the entry enabled, and `hermes mcp list` read by a program shows the row; the lines end in CR LF. |
| W12 | [observed] The first command of Hermes in a data folder it never used took 47 s; the next ones about 2 s. With a limit of 30 s the registration of the trial failed the first time and passed when the same command was repeated. The limit of the connector is 90 s since then. |
| W13 | [observed] The whole command of the page, then the approval, then the connector started with the arguments of the saved entry: the hub showed the Hermes of the trial as active and `arauto_status` answered connected. |

Still not checked: a real agent on Windows following the page by itself; whether that Hermes
loads the new entry without `/reload-mcp`; anything at all on a Mac.

## The key of the local API through the entry (15/11/2026)

**Checked on 15/11/2026, on Hermes Agent v0.22.5, Docker install, Linux, by hand on one Hermes.**
Marking as at the top. Used by
[decision 0046](../docs/decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md).

| | Fact |
|---|---|
| K1 | [observed] [code] Hermes's local API listens (`127.0.0.1:8642` by default) only when its variable `API_SERVER_KEY` holds a usable secret, at least 16 characters; otherwise the adapter does not start. `gateway/config_env.py`, `gateway/config.py`. |
| K2 | [observed] `hermes mcp add NAME --command PATH --env 'KEY=${VAR}' --args ...` accepts `--env` (`KEY=VALUE`, several allowed; `--args` must be the last option) and saves the entry with an `env:` map holding the literal reference. |
| K3 | [observed] [code] At spawn Hermes resolves `${VAR}` (and `${env:VAR}`) in the entry's `env` from its own secret scope and hands the resolved value to the stdio child. With the entry env `ARAUTO_HERMES_API_KEY: ${API_SERVER_KEY}` the connector process had `ARAUTO_HERMES_API_KEY` with 64 characters, not the literal. A reference to an unset or empty variable is treated as absent: the child does not get the variable at all. `tools/mcp_tool_config.py` `_interpolate_env_vars`, `_build_safe_env`. |
| K4 | [observed] With that in place, a note sent on a link with the wake mark woke the Hermes: the hub's notice was confirmed at the first attempt in the same second, the log of that Hermes shows an inbound run on its API server with the fixed sentence, and the Hermes read the note and answered it within 8 seconds, with no person talking to it. The sender, which had no key, got a notice that stayed pending after 3 attempts. |
| K5 | [observed] A **running** gateway does not reload an MCP server whose entry changed, nor restart one whose process died: the edited entry was not applied, the killed connector was not started again in 15 minutes, and `hermes mcp remove` followed by `hermes mcp add` did not bring it back either. It came back, with the new env, only after the gateway was restarted (Hermes says a new session or `/reload-mcp` loads it into sessions). The first registration of a new name, made from inside a session, was picked up within a second. |
| K6 | [observed] The child also receives variables that Hermes's secret source injects for every stdio child (one provider API key was seen in the connector's environment, by name). The connector must never read, log or forward any variable other than its own. |
| K7 | [observed] `arauto-connector status` run by hand reported "not configured" for the key of the local API, after the change. **Cause not established:** at that date `status` asked the leader over the local socket and did not read its own environment; the leader may have been the copy started from the old entry (K5). |
| K8 | [observed] In that install the home of the connector was given with `--home` (the args of the entry are `mcp --home PATH`), and `hermes mcp add` asks questions on the terminal which the connector already answers ([11](../docs/11-conector.md), section 4). |
| K9 | [observed] One Hermes whose model refused the connect message cited a local skill line ("use manage_connections ...; never edit mcp_servers manually"). `manage_connections` is a built-in tool of v0.22.5 for catalog apps and cannot add a local stdio server. The same command run in a terminal by the operator connected it. **Known limit of the connect-by-message path.** |

Not checked: Windows (the launcher `hermes.exe` handing its command line to Python unchanged),
macOS, v0.23 and other versions; the handling of `$` in arguments by the shim
`/opt/hermes/bin/hermes`; an API on another port; whether Hermes's masking of the terminal
output hides the name `API_SERVER_KEY` in printed lines.

## A background process with notify, and answers between agents (15/11/2026)

**Hermes Agent v0.22.5, Docker install, Linux, 15/11/2026.** Marking as at the top: [code] was read
in the source of Hermes and not run; [observed] was seen on a running Hermes. Used by
[decision 0048](../docs/decisoes/0048-resposta-na-conversa-que-perguntou.md). No name of a person,
company or server is recorded here.

| | Fact |
|---|---|
| N1 | [code] The terminal tool with `background=true` and `notify=true`: when the process ends, the gateway puts a message into the same conversation with the exit code, the command and the last part of the output (about 2000 characters, masked), and starts a turn. If a turn is running, the message is queued. |
| N2 | [code] `notify` is refused (`notify_unsupported`) in runs of the API server, one-shot runs, cron jobs and Kanban workers. A run created by a wake through the local API is of the first kind. |
| N3 | [code] A background process with notify is dropped after `/new`, and killed by the sweeps at the end of a conversation and at compression unless it was started with `persist_on_release`. It may ask for approval. At most 64 processes. |
| N4 | [code] Terminal processes get variables that name the conversation; a stdio tool server gets none. MCP notifications, sampling and elicitation do not start a turn. |
| N5 | [observed] On a conversation of the WhatsApp gateway, a background process of 40 seconds with notify: when it ended, the Hermes spoke by itself in the same conversation, with the exit code and the output. No approval request was seen. |
| N6 | [observed] A Hermes woken by a note for a peer read it and wrote its answer as plain text into the API run, which nobody reads. It sent no note back. |
| N7 | [observed] The asking Hermes, in a WhatsApp conversation with its operator, said it had no answer yet and ended its turn. A later answer would wake it in a separate run that the operator does not see. |
| N8 | [observed] 16/11/2026, after the connector of decision 0048 was installed (Hermes Agent v0.22.5, Docker, Linux, one pair of Hermes, one trial): the operator asked one Hermes, in a WhatsApp conversation, to ask another. The note was sent, the other Hermes was woken, read it 5 s later and answered by a note 12 s after the question, with no person asking it to. The asking Hermes read the answer 12 s after it arrived and gave it to its operator in the same conversation. Both notices were confirmed at the first try. Not seen from the hub: whether the answer reached the asking Hermes through `await-reply` or through a wake of its local API, and whether it was told twice. |
| N9 | [observed] 16/11/2026, Hermes Agent v0.22.5 on Windows (commands in Git Bash), one trial: with the connector before decision 0048, the asking Hermes waited with a sleep of its own and reported that no answer had come, although the answer had arrived 42 s after the question and had been marked read 8 s later, while that conversation slept; the reading was not seen, and a separate run woken by the notice is the likely reader. With the connector of decision 0048, the same question from the same Hermes had its answer 7 s later, read 13 s after that and given to the operator in the same conversation. The program was replaced while the old one ran (a rename, which Windows allowed), and the gateway was restarted by the operator. |
| N10 | [observed] 16/11/2026, Hermes Agent v0.22.5: the official Docker image creates `API_SERVER_KEY` in the `.env` of the data folder at first boot, as seen in a fresh container. That it skips this when the container environment already carries one was read in `docker/stage2-hook.sh`, not tried. |
| N11 | [observed] 16/11/2026, Hermes Agent v0.22.5, in a Docker container (Linux): `hermes config get NAME` exits 0 when the setting is set and 1 when it is not; a variable of the process environment counts as set. |
| N12 | [observed] 16/11/2026, Hermes Agent v0.22.5, in a Docker container (Linux): the terminal tool's sanitizer removes every name that begins with `API_SERVER_` from the environment of the commands it runs. |
| N13 | [observed] 16/11/2026, Hermes Agent v0.22.5, in a Docker container (Linux): `hermes config set API_SERVER_KEY VALUE` exits 0, does not echo the value and writes only the `.env` of the data folder (mode 600). |
| N14 | [observed] 16/11/2026, Hermes Agent v0.22.5 on Windows, gateway started by a scheduled task, one trial: `/restart` sent in the chat brought up a new gateway process and a new connector leader within seconds, and the local API answered. |
| N15 | [observed] 16/11/2026, Hermes Agent v0.23.2, official Docker image (tag `stable`), one trial each. The image makes `API_SERVER_KEY` at the first start, as N10 says of v0.22.5, and makes it again each time the container is created without one. The gateway runs under a second supervision tree (`/run/hermes-service/gateway-default`); restarting that service alone does not run the start hook again, so a removed key stays removed. |
| N16 | [observed] 16/11/2026, same Hermes, key removed and gateway restarted (local API off). The connection block was pasted in the chat of the Hermes dashboard and Hermes ran `join` itself. `join` printed the verification code, then the sentences that a key was made and that the gateway must restart; `.env` held one `API_SERVER_KEY` line, mode 600; the local API stayed off. After the approval and one gateway restart, the local API answered and `status` said the key was received. The container did not restart, so the key in use was the one the connector made. |
| N17 | [observed] 16/11/2026, same Hermes. `/restart` typed in the chat of the Hermes dashboard was refused: "/restart is not available on the shared gateway yet". The Restart Gateway button of the dashboard restarted the gateway alone. N14 saw `/restart` work in another chat of v0.22.5; the two are different chats and different versions. |
| N18 | [observed] 16/11/2026, same Hermes, one trial. With the image-made key present, `join` printed no sentence about making a key and `.env` did not change. With the connector-made key, a note sent to another Hermes was answered and the answer woke this Hermes (`status`: last wake accepted by the local API); the wake opened a new session of the local API, not the dashboard chat that had asked. |
| N19 | [read in the code] 17/11/2026, Hermes Agent v0.22.5, Docker image, files `scripts/whatsapp-bridge/bridge.js` and `gateway/pairing.py`. Who may talk to a Hermes over its own WhatsApp bridge is decided by three settings of the `.env` of the data folder (mode 600): `WHATSAPP_MODE` (`bot` or `self-chat`), `WHATSAPP_DM_POLICY` (`allowlist`, `pairing` or `open`) and `WHATSAPP_ALLOWED_USERS` (telephones separated by commas; `*` opens it to everyone). With the policy `pairing`, the senders approved later are kept in `platforms/pairing/whatsapp-approved.json` of the data folder. So the list of the `.env` alone is not the whole set of senders a Hermes accepts. |
| N20 | [read in the code] same version and files. In `self-chat` mode the bridge accepts only the messages the account sent to itself (`fromMe`) and refuses every other sender: whoever can send as that account gives the orders. In `bot` mode a `fromMe` message that is not an echo of the bridge's own send is dropped, unless `WHATSAPP_FORWARD_OWNER_MESSAGES` is on, in which case it is forwarded as typed by the owner. Not tried: what the bridge does with a message sent through another linked device of the same account. |
| N21 | [observed] 17/11/2026, four Hermes on one server (three v0.22.5, one v0.23.0), names of settings only, values not read: three had the bridge on in `bot` mode, two with the policy `pairing` and one with `allowlist`; one had no WhatsApp setting. `tools/environments/local_env_policy.py` lists `WHATSAPP_MODE` and `WHATSAPP_ALLOWED_USERS` among the names it handles for the commands of the terminal tool; whether it removes them was not checked. |
| N22 | [observed] 17/11/2026, two Hermes in Docker (v0.22.5 and v0.23.0), read-only, values masked. The running connector (`arauto-connector mcp`, started by Hermes as the user `hermes`) has no `WHATSAPP_*` name in its environment. The same user can run `hermes config get WHATSAPP_ALLOWED_USERS` and `hermes config get WHATSAPP_MODE`: both exit 0 and print the value on the last line (the list of telephones separated by commas; `bot`), after a line of notice from Hermes. The `.env` of the data folder is readable by that user. Not tried: the same call made by the connector itself; Windows. |
| N23 | [observed] same day, v0.22.5, policy `pairing`. `platforms/pairing/whatsapp-approved.json` is readable by the user `hermes` (mode 600). It is a map: each key is an identifier of 15 digits, each value has `approved_at` and `user_name`. Whether that identifier is a telephone or a hidden identifier of WhatsApp was not determined; 15 digits is longer than the telephones seen. The Hermes with the policy `allowlist` has no such file. Where the bridge keeps the telephone of its own account was looked for: only in the folder `session` (credentials, not opened) and in the memory of the bridge (`sock.user.id` in `bridge.js`); no file meant to be read was found. |
| N24 | [observed] 18/11/2026, reported by the maintainer, one production Hermes, Hermes Agent version not recorded, one occurrence. Its own WhatsApp bridge was a number of the hub, with telephones recorded as giving it orders (decision 0051). Asked to send a notice to one of them, it called whatsapp_send_text through Arauto three times, got AR-705 each time, told its operator that Arauto did not let it send, and stopped. Told to use its own WhatsApp, it delivered at the first try. Not checked: whether a Hermes reads a new error sentence or a tool description in time to avoid the first call; whether Hermes passes MCP server instructions to the model; whether a run woken through the local API can write through its own WhatsApp. |
| N25 | [observed] 18/11/2026, Hermes Agent v0.23.0 and v0.22.5, the production images run as throwaway offline containers (`docker run --rm --network none`) with a temporary `HERMES_HOME` holding only a small `config.yaml`, no real data. (1) `hermes config get platform_toolsets.api_server`, with the list written in block style or in flow style (`[a, b]`) in `config.yaml`, prints on stdout the YAML block form, one item per line: the bytes are exactly `  - a\n  - b\n` (two spaces, `-`, one space, the name); nothing on stderr; exit 0; the same on both versions. (2) With `api_server: []` it prints `[]\n`, exit 0. (3) With no `api_server` row: exit 1 and the text `Config key not set: platform_toolsets.api_server` (seen with both streams merged; which stream was not checked). (4) `hermes config set platform_toolsets.api_server '["web","arauto"]'` exits 0 and stores a real YAML list of two strings (block form in `config.yaml`); `config get` then prints `  - web\n  - arauto\n`; the same on both versions. (5) Same day, on one production Hermes v0.23.0 in Docker, read only, as the user `hermes`: the same `config get` printed on stdout only the block lines of its list, and the command helper's line (`Command helper: applied 1 secret`) went to stderr. The connector reads stdout alone, and still accepts plain lines before the run (N22 saw a notice with both streams merged). Not tried: whether a running gateway needs a restart to apply the new list, and what an empty list means for a woken run (steps 5 and 6 of P6). |
| N26 | [observed] 19/11/2026, one production Hermes Agent v0.23.0 in Docker, as the user `hermes`, each command run against a temporary copy of its `config.yaml` (`HERMES_HOME` pointing to the copy), the real file not touched. (1) `hermes tools enable web`, with no `--platform`, applies to `cli` (the help says "default: cli"); `cli` already had `web`, and the copy did not change. (2) `hermes tools enable --platform whatsapp terminal web` turned `whatsapp: []` into the list `terminal`, `web` and printed `Enabled: terminal, web`. (3) The names of MCP servers are refused there: `Unknown toolset 'arauto'`, and the same for its other MCP server, and the rest is still applied. The same Hermes, with `whatsapp: []`, reported having the tools of its MCP servers in that chat and no terminal or web. With P1 (an explicit list without `arauto` gave a woken run no Arauto tools), `enable` on an empty list therefore probably removes the MCP servers from that platform; not tried on a running gateway. `hermes config set platform_toolsets.<platform> '[...]'` (N25) is the way to write a list that names them. `hermes tools` with no subcommand and `--summary` refuse to run without an interactive terminal. |

K4 above saw another Hermes answer a note by note within 8 seconds, so the behavior differs between
Hermes and between models. N6 and N7 are one pair of Hermes on one day.

Not checked: whether a note that arrives while a turn runs is queued as N1 says; the TUI and the
dashboard; the approval request; whether the 180-second command limit applies to a background
process; compression, `/new`, a gateway restart and `persist_on_release`; the size and the masking
of the output tail; Windows, macOS, v0.23 and other versions; whether a model follows the text the
connector adds to a result.

## The tools of a woken run (18/11/2026)

Reported by the maintainer, one production Hermes and three others on the same server. Marking as at the top. Used by [decision 0046](../docs/decisoes/0046-chave-pela-entrada-e-hermes-sem-chave.md), section of 18/11/2026. No name of a person, company or server is recorded here.

| | Fact |
|---|---|
| P1 | [observed] 18/11/2026, Hermes Agent v0.23.0, Docker, one Hermes: `config.yaml` had an explicit per-platform list `platform_toolsets.api_server` with two entries, another MCP server of that Hermes (named by its key under `mcp_servers`) and `web`. The entry `mcp_servers.arauto` (command the connector, args `mcp --home ...`) was present and enabled. A configuration backup of 15/11/2026 already had that list, before waking was set up. |
| P2 | [observed] same Hermes: every peer notice was answered `accepted` (the local API, `POST /v1/runs`, answered 202), and each started a run with platform `api_server` and an empty history. Each run made one model call and ended with a text answer, with no tool turn: it never called `hermes_list_threads` or `hermes_read_thread`. The notes stayed unread until the operator talked to the Hermes on its usual channel, where the Arauto tools were present. |
| P3 | [observed] same Hermes: `arauto_status` said `local_waking: true` meanwhile. |
| P4 | [observed] same Hermes: fixed by hand by appending `arauto` to that list and restarting the gateway (supervised by s6; its run file runs `hermes gateway run --replace`). On 19/11/2026, after that fix and the restart, another Hermes sent it a note through Arauto asking for a short answer: the notice woke a run (platform `api_server`) that called `hermes_list_threads`, `hermes_read_thread` (twice) and `hermes_send_message`, and the answer reached the sender about 7 seconds after its send (3 tool turns). One trial, one Hermes; the effect of the list without the restart was not tried. |
| P5 | [observed] same day, three other Hermes (v0.22.5) on the same server: two have no `api_server` row under `platform_toolsets`; one has `api_server: []`. |
| P6 | [to check] What an empty list means to Hermes (no tools, or the default); whether a running gateway applies a changed list without a restart; whether the toolset of an MCP server is always named by its key under `mcp_servers`. The source of Hermes was not read for this. How `config get` prints a list and how `config set` stores one are settled by N25. |
