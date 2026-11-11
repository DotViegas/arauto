# Fatos conferidos no Hermes

**Estado: fatos conferidos**, com a origem de cada afirmação. O que o desenho do Arauto assume
sobre o Hermes Agent.

**Conferido em 09/11/2026, no Hermes Agent v0.22.5 (2026.9.24), em container Linux x86_64.** A versão
0.23.0 também é alvo e ainda não foi conferida ponto a ponto.

Marcação: **[código]** lido no código instalado; **[observado]** visto num Hermes em funcionamento;
**[a conferir]** suposição ainda sem teste.

## Servidores de ferramentas (MCP)

| Fato | Origem |
|---|---|
| O cliente MCP fala stdio, HTTP e SSE. Não há websocket como transporte. | [código] `tools/mcp_tool.py` |
| Há OAuth para servidores MCP, inclusive o fluxo de aparelho (RFC 8628), iniciado só por `hermes mcp login`. | [código] `tools/mcp_oauth_device.py` |
| Comando oficial: `hermes mcp add NOME --command PROGRAMA --args ...` ou `--url ENDERECO --auth oauth\|header`. Também `remove`, `list`, `test`. | [observado] ajuda do comando |
| A lista de servidores é reconciliada sozinha, em menos de um minuto, sem reiniciar. | [observado] |
| Um servidor MCP aparece em todos os meios de conversa do Hermes, salvo configuração por meio. | [observado] |
| Toda entrada de servidor é conferida ao salvar e ao iniciar. É recusada a que usa interpretador de comandos com chamada de rede ou com indício de envio de dados. | [código] `hermes_cli/mcp_security.py` |
| O servidor stdio recebe um ambiente **filtrado**: pasta do usuário, caminho de programas, pastas temporárias e as variáveis da própria entrada. Não herda o ambiente do Hermes. | [código] `_build_safe_env`; [observado] num servidor em uso |
| Ao encerrar, o Hermes fecha a entrada do servidor, espera 2 segundos, pede o término e só depois força. Um supervisor encerra os servidores se o Hermes morrer. | [código] SDK do MCP e `tools/mcp_death_supervisor.py` |
| O servidor stdio vive enquanto vive o processo do Hermes que o iniciou. | [observado] 10 dias sem reinício |
| Há tratamento para o aviso de que a lista de ferramentas mudou. | [código] `tools/mcp_tool_health.py` |

## Terminal da LLM

| Fato | Origem |
|---|---|
| O terminal roda na própria máquina, com o **mesmo usuário** do Hermes. | [observado] configuração e processos |
| A pasta do usuário é o volume de dados do Hermes, com leitura, escrita e execução. | [observado] |
| Cada comando tem limite de 180 segundos. | [observado] configuração |
| Antes de rodar, o Hermes pede aprovação humana para: conteúdo da internet enviado direto ao interpretador (`curl ... \| sh`), acesso ao arquivo de segredos do Hermes, arquivos de credencial conhecidos e variáveis com nome de segredo. | [código] `tools/approval_detection.py` |
| A saída das ferramentas tem segredos mascarados, por padrão de formato e por nome de variável. | [código] `agent/redact.py` |
| `curl`, `sha256sum`, `flock` e `tar` existem na imagem. | [observado] |
| Um processo não lê a memória de outro que não seja seu descendente. | [observado] no servidor conferido; depende do sistema |

Consequência para o projeto: tudo o que o conector gravar na pasta do usuário, a LLM consegue ler.

## Processos

| Fato | Origem |
|---|---|
| Os serviços do container são supervisionados com privilégio de administrador. O usuário do Hermes não registra um serviço novo que sobreviva a reinício. | [observado] |
| Num Hermes com um servidor stdio configurado havia **uma** cópia dele, filha do processo de mensagens. | [observado] |

## API local

| Fato | Origem |
|---|---|
| `POST /v1/runs` inicia uma execução. Exige chave própria. Por padrão escuta só no endereço local. | [observado] |
| A chave da API não fica no ambiente do processo: o Hermes a lê do arquivo de segredos. | [observado] só nomes de variáveis |
| Existe suporte a identificador de repetição na API. | [código] `gateway/platforms/api_server.py` |
| O contexto de cada conversa informa a origem: a plataforma e com quem. | [código] `gateway/session.py` |

## Canal nativo de WhatsApp

| Fato | Origem |
|---|---|
| Conversa direta aceita as políticas pareamento, lista, aberta e desligada. Só a política de lista restringe a quem está na lista. | [código] `gateway/platforms/access_policy_mixin.py` |
| Grupo só é encaminhado ao Hermes com política de lista ou aberta. | [código] idem |

## A conferir

A lista única do que precisa ser provado está em [docs/12-roteiro.md](../docs/12-roteiro.md), Fase 1.
Estas são as suposições sobre o Hermes:

| Suposição | Por que importa |
|---|---|
| O registro do conector pelo comando oficial passa pela conferência de entradas. | Sem isso não há modo rápido. |
| Quantos processos do Hermes iniciam o servidor quando painel e rotinas agendadas estão ativos. | Define o trabalho do líder. |
| A entrada do servidor aceita referência a uma variável do Hermes no campo de ambiente. | É o caminho mais limpo para a chave da API local. |
| O bloco dispara ou não o pedido de aprovação do terminal. | O fluxo funciona nos dois casos; a tela precisa dizer o que esperar. |
| O Hermes guarda procedimentos em memória ou skill a partir do que executa. | O bloco pede que não guarde. |
| Endereço, corpo e identificador de repetição da chamada à API local. | É por ela que o Hermes é acordado. |
| O Hermes reinicia um servidor de ferramentas que encerra. | É como o conector troca de versão num reconectar. |
| O comportamento da versão 0.23.0 em cada linha deste arquivo. | A 0.23 abre mais ferramentas por padrão. |
