# 0039 — A matriz de acessos e as ferramentas de WhatsApp no primeiro MVP

**Estado:** proposta · **Data:** 12/11/2026, com o código da matriz e das ferramentas no hub (M3,
segunda parte)

## Contexto

O [08-permissoes.md](../08-permissoes.md) dá os quatro verbos, o escopo, a linha de conversa e a
regra de decisão; o [09-contrato-mcp.md](../09-contrato-mcp.md) dá a lista das ferramentas, mas
deixa para o MVP os argumentos e o formato das respostas; o [06-modelo-de-dados.md](../06-modelo-de-dados.md),
seção 5, dá as tabelas. Esta decisão registra o que o código escolheu onde os textos são omissos,
e onde se afastou deles. Tudo foi testado só contra o motor falso.

## Decisão

### A matriz

- **Tabelas** `access` (uma por par Hermes × número: `scope` `all`/`list`, `can_read`,
  `can_send`, `can_modify`, `can_wake`, `can_start_chats`, `created_by`, `created_at`,
  `changed_at`) e `access_chat` (a linha: os quatro verbos por conversa), na migração 0010. O banco
  recusa "ser acordado" sem "ler", "pode iniciar conversa" sem "enviar" e escopo "todas", e linha de
  conversa de outro número (chave estrangeira composta com a conversa e o número).
- **Os verbos do número** valem para o que é do número (estado, listagens, conferir telefone) e,
  com escopo "todas", em toda conversa sem linha. Com escopo "lista", só existem as conversas com
  linha. A linha substitui por inteiro; linha sem verbo bloqueia.
- **A família de ferramentas** de um verbo é liberada quando o Hermes tem esse verbo em algum
  número, na linha de alguma conversa inclusive. As listagens (`whatsapp_list_chats`,
  `whatsapp_list_groups`) e o `whatsapp_status` exigem "ler" em algum lugar do número e mostram só
  as conversas que o Hermes pode ler; `whatsapp_check_number` exige "enviar" no próprio número.
- **Conversa que "ainda não existe"** (regra 5): sem arquivo de mensagens, é a conversa que o hub
  não tem e em que o motor não tem nenhuma mensagem. Ela exige escopo "todas", "enviar" e "pode
  iniciar conversa"; senão AR-701 (com "todas") ou AR-705 (com "lista").
- **Número trocado para os acessos**: não libera ferramenta nenhuma, e toda chamada que o nomeia
  recebe AR-702, até `admin number confirm` ou `unpair`.
- **Fim dos acessos**: remover o número ou o Hermes apaga os acessos dele na mesma gravação, cada
  um com o seu evento.
- **Auditoria**: `access.granted`, `access.changed`, `access.removed`, `access_chat.set` e
  `access_chat.removed`, todos eventos de segurança, com o antes e o depois (escopo, verbos e
  "pode iniciar"). Gravados por `begin_security`: banco restaurado não recebe nada.
- **Comando**: `arauto-hub admin access grant HERMES NUMERO --scope all|list [--read] [--send]
  [--modify] [--wake] [--start-chats]` (o acesso inteiro; o escopo é obrigatório),
  `access remove`, `access chat HERMES NUMERO CONVERSA [verbos]`, `access chat-remove` e
  `access list [HERMES]`. A conversa é um telefone (o motor dá a forma exata, com ou sem o nono
  dígito; sem o motor no ar, recusa), um identificador do motor (`...@g.us`, `...@c.us`, `...@lid`)
  ou o identificador do hub mostrado por `access list`. O comando avisa, e não impede, quando dá
  "enviar" num número marcado como de comando e quando outro Hermes também envia ou é acordado no
  mesmo número (08, seções 5 e 6).

### A lista de ferramentas

- O `session` leva o que a matriz libera agora. A cada mudança, a sessão manda `tools`: ela é
  cutucada pelo aviso do comando (`hermes:{id}`, também para cada Hermes de um número removido) ou
  pela varredura, que compara, numa consulta por organização, o que cada sessão recebeu com o que o
  Hermes tem. Um `number:{id}` pede a varredura na hora.
- **Nenhuma chamada confia na lista**: cada chamada de WhatsApp lê o acesso no banco, numa consulta
  por índice, e a linha da conversa quando precisa.

### Identificadores

- **`number`** é o apelido. Omitido, vale o único número do Hermes; com mais de um, AR-711. Apelido
  de número sem acesso e apelido inexistente dão a mesma resposta (AR-701).
- **`chat`** é o UUID da linha `chat` do hub. A listagem cria a linha da conversa que o hub ainda
  não tem (só com escopo "todas": com "lista", conversa sem linha não aparece).
- **`to`** é um telefone (com o código do país; o hub pergunta ao motor a forma exata) ou um UUID de
  conversa. Identificador do motor (`...@c.us`) não é aceito.
- **`message_id`** é assinado pelo hub: base64url de `conversa (16 bytes) | HMAC-SHA-256 (32) |
  identificador no motor`, com a chave dos resumos de WhatsApp da organização. Não é cifrado: leva
  a conversa e o autor, que a leitura já mostra; é o mesmo a cada leitura. Assinatura errada, de
  outra organização, de conversa de outro número ou fora do escopo: AR-712. A mensagem nomeia a
  conversa dela, e o hub confere que é a conversa assinada.

### O teto de envio

- 20 em qualquer janela de 60 segundos, por Hermes e por número, no relógio do hub. Contam
  `whatsapp_send_text`, `whatsapp_send_file`, `whatsapp_react`, `whatsapp_forward` e
  `whatsapp_check_number` (que pergunta ao WhatsApp sobre gente de fora); marcar como lida e
  "digitando" não contam. Conta a chamada que passou por todas as conferências do hub e vai ao
  motor, mesmo que o motor falhe; recusa do hub não conta.
- A recusa é AR-704 com `retry_after_s` (quando o mais antigo sai da janela). A frase do AR-704 com
  espera entrou no catálogo, como a do AR-706.
- A conta vive na memória do hub, sob uma trava: de muitas chamadas ao mesmo tempo passam
  exatamente as que cabem; outra sessão do mesmo Hermes acha a mesma conta. Reiniciar o hub zera a
  conta; um segundo hub da mesma organização não é tratado.

### Código novo no catálogo

- **AR-712**: "Message not found: it does not exist, is outside what this Hermes can use, or
  WhatsApp no longer allows it." Usado para identificador de mensagem que não vale e para mensagem
  que o motor não acha ou não deixa mais mudar.

### As ferramentas e as respostas

| Ferramenta | Argumentos | Resposta |
|---|---|---|
| `whatsapp_status` | `number?` | `number`, `state`, `connected`, `whatsapp_limits` (`message_capping`, `reachout_timelock`) ou `null` |
| `whatsapp_list_chats` | `number?`, `limit` (1–100, 20), `offset` | `number`, `chats` [`chat`, `kind`, `name`, `phone`, `last_message`], `more`, `next_offset` |
| `whatsapp_read_chat` | `number?`, `chat`, `limit` (1–200, 30), `before`, `after` (segundos) | `number`, `chat`, `messages` (a mais nova primeiro), `more`, `next_before` |
| mensagem (na leitura) | | `message_id`, `time` (segundos), `from_me`, `author` (`+telefone` ou `null`), `text`, `file` (`mimetype`, `filename`) ou `null`, `reply_to` |
| `whatsapp_get_contact` | `number?`, `chat` (de contato) | `chat`, `phone`, `name`, `push_name`, `about`, `picture_url`, `is_business` (`null`) |
| `whatsapp_download_media` | `number?`, `message_id` | `mimetype`, `filename`, `size`, `data` (base64), até 7 MB |
| `whatsapp_list_groups` | `number?`, `limit`, `offset` | `groups` [`chat`, `subject`, `participants`], `more`, `next_offset` |
| `whatsapp_get_group` | `number?`, `chat` (de grupo), `limit` (1–200, 100), `offset` | `chat`, `subject`, `description`, `total`, `participants` [`phone`, `role`], `next_offset` |
| `whatsapp_check_number` | `number?`, `phone` | `phone`, `has_whatsapp`, `exact` |
| `whatsapp_send_text` | `number?`, `to`, `text` (até 64 KiB), `reply_to?`, `mentions?` (até 50 telefones) | `chat`, `message_id` |
| `whatsapp_send_file` | `number?`, `to`, `kind` (`image`/`document`/`voice`), `mimetype`, `filename?`, `data` (base64, até 3 MB), `caption?` | `chat`, `message_id` |
| `whatsapp_mark_read` | `number?`, `chat` | `done` |
| `whatsapp_set_typing` | `number?`, `chat`, `on` | `done` |
| `whatsapp_react` | `number?`, `message_id`, `reaction` (vazio tira) | `done` |
| `whatsapp_forward` | `number?`, `message_id`, `to` | `chat`, `message_id` |
| `whatsapp_edit_message` | `number?`, `message_id`, `text` (obrigatório) | `done` |
| `whatsapp_delete_message` | `number?`, `message_id` | `done` |

Toda resposta leva `number`. A leitura corta em mensagens inteiras dentro de 256 KiB; quando ainda
há mensagens, a página termina numa troca de segundo, para que `next_before` (exclusivo) não perca
nenhuma. Editar e apagar exigem o identificador completo, que só vem de mensagem lida no formato
completo; um identificador só do WhatsApp é recusado com AR-711.

**`arauto_status`**, cada item de `numbers`: `number` (apelido), `state`, `scope`, `verbs` (os do
número), `chat_verbs` (os que alguma linha de conversa dá) e `can_start_chats`. Nada de telefone ou
de nome da sessão do motor. Número removido não aparece; número trocado aparece com o estado.

## Consequências

- O `docs/11-conector.md`, seção 8, precisa da linha do AR-712, e o `docs/09-contrato-mcp.md` do
  formato acima; ficam para a junção com a linha do conector.
- Sem arquivo de mensagens, edição, apagamento e reações não aparecem na leitura (o motor não os
  traz na listagem de mensagens), e toda leitura depende do motor no ar.
- Uma leitura de 200 mensagens longas pode passar do teto de 4 MiB da resposta do motor e virar
  AR-703.
- Alternativas consideradas: identificador de mensagem cifrado (opaco de fato, mas diferente a cada
  leitura, o que impede o Hermes de reconhecer o que já leu); identificador cru do motor (o Hermes
  poderia escrever um de outra conversa); teto no banco (sobrevive ao reinício, mas põe uma gravação
  em cada envio).
