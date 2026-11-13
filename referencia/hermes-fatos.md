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
| Official command: `hermes mcp add NAME --command PROGRAM --args ...` or `--url ADDRESS --auth oauth\|header`. Also `remove`, `list`, `test`. | [observed] help of the command |
| The list of servers is reconciled by itself, in less than a minute, without restarting. | [observed] |
| An MCP server appears on all of Hermes's conversation platforms, unless configured per platform. | [observed] |
| Every server entry is checked on saving and on starting. One that uses a command interpreter with a network call or with a sign of data being sent out is refused. | [code] `hermes_cli/mcp_security.py` |
| The stdio server receives a **filtered** environment: the user's folder, the program path, temporary folders and the variables of the entry itself. It does not inherit Hermes's environment. | [code] `_build_safe_env`; [observed] on a server in use |
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
| The registration of the connector through the official command passes the check of entries. | Without this there is no quick mode. |
| How many Hermes processes start the server when panel and scheduled routines are active. | It defines the work of the leader. |
| The server entry accepts a reference to a Hermes variable in the environment field. | It is the cleanest path to the key of the local API. |
| Whether the block triggers the terminal's approval request or not. | The flow works in both cases; the screen needs to say what to expect. |
| Hermes keeps procedures in memory or as a skill from what it executes. | The block asks it not to keep them. |
| Address, body and repetition identifier of the call to the local API. | It is through it that Hermes is woken. |
| Hermes restarts a tool server that exits. | It is how the connector changes version on a reconnect. |
| The behavior of version 0.23.0 on each line of this file. | 0.23 opens more tools by default. |
