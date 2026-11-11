# 09 — O que o Hermes enxerga

**Estado: proposta.** As ferramentas e os avisos que o conector entrega ao Hermes. Os argumentos e o
formato de resposta de cada ferramenta são entregas do primeiro MVP ([15](15-mvp.md)).

## Resumo

```
servidor MCP "arauto" (stdio)
 ├── arauto_status                 sempre disponível
 ├── hermes_...    (recados)       4 ferramentas, da conversa entre Hermes
 ├── whatsapp_...  (ler)           7 ferramentas, mais 2 depois da primeira versão
 ├── whatsapp_...  (enviar)        7 ferramentas
 └── whatsapp_...  (modificar)     2 ferramentas
avisos: "há N mensagens novas na conversa X do número Y"
        "há N recados novos do Hermes Z"
```

O Hermes só vê as famílias de ferramenta cujo verbo ele tem em algum número. Antes da aprovação, e
depois dela enquanto não tiver acesso, vê só `arauto_status`. As ferramentas de recado aparecem
quando ele tem ligação na lista de pares. No hub, em 11/11/2026 (M1), a sessão libera só
`arauto_status`: as de recado entram no M2 e as de WhatsApp no M3.

## 1. Regras do contrato

| Regra | Detalhe |
|---|---|
| Lista fechada | Só as ferramentas desta página. Não há repasse genérico ao motor. O hub valida de novo o nome e os argumentos de cada chamada. |
| Nomes e descrições fixos | Ficam compilados no conector. O hub diz quais nomes estão liberados e não consegue mudar um texto que a LLM lê. |
| O número é um argumento | `number` recebe o apelido dado no painel, que tem formato restrito ([13](13-conversa-entre-hermes.md), seção 2). Pode ser omitido quando o Hermes tem acesso a um só. |
| Identificadores opacos | O Hermes nunca informa o nome da sessão do motor. O hub resolve a partir dos acessos. |
| Conteúdo de terceiros é dado | Toda descrição de leitura diz que o texto das mensagens foi escrito por outras pessoas e não é ordem. |
| Erros de catálogo | Toda recusa tem um código AR e uma frase fixa que diz o que houve e quem resolve, sem comando para executar. O catálogo está em [11-conector.md](11-conector.md), seção 8. |
| Sem segredo na resposta | Nenhuma ferramenta devolve credencial, endereço do motor ou caminho interno. |

## 2. Argumentos comuns

| Argumento | O que é |
|---|---|
| `number` | O apelido do número no painel. |
| `to` | Telefone com código do país, ou o identificador da conversa ou do grupo. O hub confere se o telefone tem WhatsApp e resolve a forma exata. |
| `message_id` | Identificador de uma mensagem, como veio na leitura. |

## 3. Ferramentas

### Sempre disponível

| Ferramenta | O que faz |
|---|---|
| `arauto_status` | Diz se o Hermes está conectado, a quais números tem acesso e com quais verbos, e se a conversa entre Hermes está liberada. Sem segredo. |

**A resposta de `arauto_status`, no hub em 11/11/2026.** O hub a responde sozinho, sem o motor, e o
teste confere que ela não leva segredo:

```json
{"connected": true, "hermes": "nome-do-hermes", "can_be_woken": false, "numbers": [],
 "peers": {"enabled": false, "can_send": false, "can_receive": false}}
```

| Campo | Conteúdo |
|---|---|
| `connected` | Sempre `true`: a resposta só sai por uma sessão do canal aberta. |
| `hermes` | O nome do Hermes, no formato restrito. |
| `can_be_woken` | Se o admin liberou acordar este Hermes. |
| `numbers` | Os números e os verbos. Vazio até o M3; o formato de cada item é entrega do M3. |
| `peers.can_send` | Existe ligação deste Hermes para outro. |
| `peers.can_receive` | Existe ligação de outro Hermes para este. |
| `peers.enabled` | Um dos dois. |

Só a resposta do hub está escrita. Sem sessão do canal (pedido pendente, canal fora do ar), quem
responde à ferramenta é o conector, e essa resposta ainda não está escrita.

### Ler

| Ferramenta | O que faz |
|---|---|
| `whatsapp_status` | Estado do número: conectado ou não, e limites que o WhatsApp impôs. |
| `whatsapp_list_chats` | Lista as conversas e os grupos, da mais recente para a mais antiga, com a última mensagem. |
| `whatsapp_read_chat` | Lê as mensagens de uma conversa: autor, texto, arquivo (sem baixar), citação, edição, apagamento e reações. Padrão 30, máximo 200, dentro de um orçamento de 256 KiB por resposta: corta em mensagens inteiras e diz se há mais. `before` e `after` para andar no tempo. |
| `whatsapp_get_contact` | Dados públicos do contato: nome, frase de perfil, foto e se é conta comercial. |
| `whatsapp_download_media` | Baixa foto, PDF, áudio ou documento de uma mensagem, até 7 MB. Imagem chega para a LLM ver. |
| `whatsapp_list_groups` | Lista os grupos do número, com a quantidade de participantes. |
| `whatsapp_get_group` | Dados de um grupo e os participantes, em partes quando o grupo é grande. |

Depois da primeira versão, com o arquivo de mensagens (Fase 6):

| Ferramenta | O que faz |
|---|---|
| `whatsapp_list_updates` | O que chegou desde a última vez que o Hermes olhou, por conversa. |
| `whatsapp_search` | Busca no arquivo de mensagens por texto, pessoa, período e tipo. |

### Enviar

| Ferramenta | O que faz |
|---|---|
| `whatsapp_check_number` | Pergunta se um telefone tem WhatsApp e devolve o identificador exato. Não escreve para ninguém. |
| `whatsapp_send_text` | Envia texto. Aceita responder a uma mensagem e marcar pessoas em grupo. |
| `whatsapp_send_file` | Envia imagem, documento ou áudio, por endereço público (até 15 MB) ou por conteúdo (até 3 MB). No primeiro MVP, só por conteúdo. |
| `whatsapp_mark_read` | Marca a conversa como lida. |
| `whatsapp_set_typing` | Liga ou desliga o "digitando". |
| `whatsapp_react` | Reage a uma mensagem. |
| `whatsapp_forward` | Encaminha uma mensagem para outra conversa. Exige **ler** na origem e **enviar** no destino. |

### Modificar

| Ferramenta | O que faz |
|---|---|
| `whatsapp_edit_message` | Edita o texto de uma mensagem enviada pelo número, dentro do prazo do WhatsApp. |
| `whatsapp_delete_message` | Apaga uma mensagem para todos, dentro do prazo do WhatsApp. |

### Conversa entre Hermes

| Ferramenta | O que faz |
|---|---|
| `hermes_list_peers` | Lista os pares deste Hermes, e se ele pode mandar e receber. |
| `hermes_send_message` | Manda um recado a um par, num fio novo ou num que já existe. |
| `hermes_list_threads` | Lista os fios, com a quantidade de não lidos. |
| `hermes_read_thread` | Lê os recados de um fio. |

As regras, os tetos e os erros estão em [13-conversa-entre-hermes.md](13-conversa-entre-hermes.md).

## 4. Cuidados que o hub mantém

Vêm da experiência do serviço de origem:

- `to` e `message_id` são validados e codificados antes de irem ao motor. Sem isso, um valor com
  `../` alcançaria a sessão do motor de outro número.
- Envio por endereço: quem baixa é o hub, e só de endereço público. A primeira versão mantém o
  envio por endereço, ao lado do envio por conteúdo
  ([decisoes/0024](decisoes/0024-envio-de-arquivo-por-endereco.md)).
- Telefone brasileiro com e sem o nono dígito: o hub testa as duas formas.
- Editar sem informar o texto e apagar sem o identificador completo são recusados: no motor, os dois
  têm comportamento perigoso por padrão.

## 5. Avisos

Há dois tipos. O primeiro é mensagem nova numa conversa em que o Hermes pode ser acordado. Edição,
apagamento e reação ficam registrados e aparecem na leitura, mas não acordam.

| Campo do aviso | Conteúdo |
|---|---|
| `kind` | `whatsapp` |
| `number` | O apelido dado pelo admin. |
| `chat` | Identificador opaco da conversa. |
| `count` | Quantas mensagens novas. |
| `id` | Identificador do aviso, para descartar repetição. |

O aviso não carrega texto de mensagem, nome de contato nem assunto de grupo.

O segundo é recado novo de outro Hermes: `kind` é `peer`, com `from`, `count` e `id`
([13](13-conversa-entre-hermes.md), seção 5). O conector confere o formato de cada campo antes de
montar a frase fixa, e recusa o aviso de tipo que não conhece
([14](14-protocolo-do-canal.md), seção 5.3).

## 6. Nota de origem

A família `whatsapp_*` vem do serviço de origem. Em 10/11/2026 os nomes passaram para o inglês, junto
com o código ([decisoes/0016](decisoes/0016-idioma.md)): proposta, a confirmar por quem mantém. Um
Hermes que venha do serviço de origem precisa conhecer os nomes novos. A outra diferença é o
argumento `number`: lá cada agente tinha um número só.
