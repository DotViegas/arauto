# 0034 — Names in English in the commands, files, modules and tools

**Status:** proposal · **Date:** 10/11/2026

## Context

The language decision ([0016](0016-idioma.md)) put in English the names in the code, the descriptions of the tools and the error messages, and left open whether the names of the `whatsapp_*` tools, inherited from the source service, also changed.

## Decision

They change. With the start of the code, everything the program names is in English:

| What | Before | Now |
|---|---|---|
| Programs | (working names) | `arauto-hub`, `arauto-connector` |
| Commands of the connector | entrar, mcp, estado, sair | `join`, `mcp`, `status`, `leave` |
| Folder and files of the connector | names in Portuguese | `~/.arauto`: `current`, `devices/`, `identity.key`, `credential.seal`, `hub.json`, `state.json`, `lock`, `channel.sock` |
| Modules of the hub | painel, inscrição, canal, caixa preta, núcleo, arquivo, motor | `panel`, `enrollment`, `channel`, `vault`, `core`, `archive`, `engine` |
| WhatsApp tools | names in Portuguese | `whatsapp_list_chats`, `whatsapp_read_chat`, `whatsapp_send_text` and the others of [09-contrato-mcp.md](../09-contrato-mcp.md) |
| Arguments | numero, destino | `number`, `to` |
| Status tool | (working name) | `arauto_status` |
| Conversation between Hermes | (new) | `hermes_list_peers`, `hermes_send_message`, `hermes_list_threads`, `hermes_read_thread` |
| Error codes | working prefix | `AR-` |
| Invite | working prefix | `arc1` |

The planning documents stay in Portuguese and use the terms of the [glossary](../glossario.md); only the names the program uses appear in English.

## Consequences

- A Hermes that comes from the source service needs to know the new names of the tools.
- The language of the fixed sentence and of the text of the block still depends on the approval of the wording of each one.
- It stays as a proposal until the maintainer confirms the names of the tools. Changing a name later is cheap while the MVP has no use.
