# 0034 — Nomes em inglês nos comandos, arquivos, módulos e ferramentas

**Estado:** proposta · **Data:** 10/11/2026

## Contexto

A decisão do idioma ([0016](0016-idioma.md)) pôs em inglês os nomes no código, as descrições das ferramentas e as mensagens de erro, e deixou em aberto se os nomes das ferramentas `whatsapp_*`, herdados do serviço de origem, também mudavam.

## Decisão

Mudam. Com o início do código, tudo o que o programa nomeia fica em inglês:

| O quê | Antes | Agora |
|---|---|---|
| Programas | (nomes de trabalho) | `arauto-hub`, `arauto-connector` |
| Comandos do conector | entrar, mcp, estado, sair | `join`, `mcp`, `status`, `leave` |
| Pasta e arquivos do conector | nomes em português | `~/.arauto`: `current`, `devices/`, `identity.key`, `credential.seal`, `hub.json`, `state.json`, `lock`, `channel.sock` |
| Módulos do hub | painel, inscrição, canal, caixa preta, núcleo, arquivo, motor | `panel`, `enrollment`, `channel`, `vault`, `core`, `archive`, `engine` |
| Ferramentas de WhatsApp | nomes em português | `whatsapp_list_chats`, `whatsapp_read_chat`, `whatsapp_send_text` e as demais de [09-contrato-mcp.md](../09-contrato-mcp.md) |
| Argumentos | numero, destino | `number`, `to` |
| Ferramenta de situação | (nome de trabalho) | `arauto_status` |
| Conversa entre Hermes | (nova) | `hermes_list_peers`, `hermes_send_message`, `hermes_list_threads`, `hermes_read_thread` |
| Códigos de erro | prefixo de trabalho | `AR-` |
| Convite | prefixo de trabalho | `arc1` |

Os documentos de planejamento continuam em português e usam os termos do [glossário](../glossario.md); só os nomes que o programa usa aparecem em inglês.

## Consequências

- Um Hermes que venha do serviço de origem precisa conhecer os nomes novos das ferramentas.
- O idioma da frase fixa e do texto do bloco continua dependendo da aprovação da redação de cada um.
- Fica como proposta até quem mantém confirmar os nomes das ferramentas. Trocar um nome depois é barato enquanto o MVP não tiver uso.
