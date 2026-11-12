# Fatos conferidos no motor (WAHA)

**Estado: fatos conferidos.** O que o Arauto assume sobre o WAHA, o programa usado como motor.

**Conferido em 09/11/2026, na imagem `devlikeapro/waha:gows-2026.10.1`, com o mecanismo interno GOWS, na
edição CORE.** O custo e a licença do WAHA são com o autor dele; este arquivo não substitui a
documentação dele.

Marcação: **[código]** lido no programa instalado; **[observado]** visto em funcionamento;
**[documentação]** lido na documentação pública do autor, sem rodar o programa (a seção "Os
formatos que o hub usa").

## Aviso

O WAHA é um cliente **não oficial** do WhatsApp. O WhatsApp pode bloquear números que o usam. O
Arauto não embute o WAHA: quem instala traz o seu.

**Licença:** o `package.json` da imagem conferida declara `UNLICENSED`. Antes de qualquer
distribuição conjunta, conferir os termos publicados pelo autor.

## O que a API permite comandar

| Grupo | Operações | Origem |
|---|---|---|
| Sessões do motor | Listar, criar, ver, atualizar, apagar, ligar, desligar, reiniciar, sair. | [código] |
| Pareamento | QR e código por número de telefone. | [código] |
| Servidor | Versão, ambiente, estado, parar. Saúde e ping. | [código] |
| Limites do WhatsApp | Consulta de teto de mensagens e de bloqueio de primeiro contato, por sessão. | [código] |

Várias sessões do motor funcionam na edição CORE, uma por número. **[observado]**

## Configuração de uma sessão do motor

| Campo | Conteúdo |
|---|---|
| `webhooks` | Endereço, eventos, segredo de assinatura, política de reenvio, cabeçalhos. |
| `ignore` | Ignorar status, grupos, canais e listas de transmissão. |
| `gows.storage` | O que o motor guarda: mensagens, grupos, conversas, etiquetas, contatos. |
| `client` | O nome com que o dispositivo conectado aparece no celular. |
| `metadata`, `proxy`, `debug` | Dados livres, proxy de saída, depuração. |

## Armadilhas

| Armadilha | Consequência para o hub |
|---|---|
| Atualizar a sessão **substitui a configuração inteira** e reinicia a sessão. | O hub só aplica quando há diferença, e sempre envia a configuração completa. |
| O que está em `ignore` nem é guardado pelo motor. | Uma sessão criada com grupos ignorados perde o histórico de grupo do pareamento. O hub cria a sessão já configurada. |
| O histórico antigo só chega **no pareamento**. Não existe "carregar mais antigas". | O arquivo do hub é o lugar permanente. |
| **Sair apaga o histórico** que o motor guarda daquela sessão. | O hub importa antes de permitir sair. |
| O histórico do pareamento não vira evento: só sai pela consulta de mensagens. | O hub importa por consulta. |
| O reenvio de eventos desiste depois de um tempo (cerca de 2 minutos na configuração usada). | Na partida, o hub busca no motor o que chegou enquanto esteve fora. |
| A contagem de participantes de grupo vem sempre zero. | Conta-se a lista de participantes. |
| O campo de origem do evento é sempre a conversa, mesmo em mensagem enviada pelo próprio número. | O hub identifica o autor por outros campos. |
| O motor recusa endereço de eventos com sublinhado no nome da máquina. | O hub é alcançado por um nome sem sublinhado. |
| A API não tem contador de mensagens não lidas. | "Novidades" é calculado pelo hub. |
| Editar sem texto e apagar sem o identificador completo têm comportamento perigoso por padrão. | O hub valida antes de chamar. |
| A documentação interativa pede credencial própria. | Não serve de catálogo em tempo de execução. |

Todas as armadilhas acima foram **[observado]** ou **[código]** na versão conferida.

## O que o motor não faz

Ligações, arquivar, fixar, bloquear contato, apagar conversa e botões.

## Regra para versões novas

Antes de trocar a versão do motor, rodar à mão o teste de contrato do hub contra um número de teste:
ler, listar grupos, enviar, e conferir o formato do evento de mensagem. O hub confere a versão do motor na
partida e acusa quando não é uma versão testada.

Como rodar o teste de contrato: seção "Teste de contrato", no fim deste arquivo.

## Os formatos que o hub usa

**Lido em 11/11/2026 na documentação pública do autor** (`waha.devlike.pro`, páginas `docs/how-to/`
de sessões, eventos, envio de mensagens, conversas, contatos, grupos, segurança, mensagens recebidas
e observabilidade). Só leitura: nada foi rodado contra uma instância. As páginas não dizem a versão
da documentação; as versões que aparecem nelas são só de exemplos (`2023.10.12`, `2024.2.3`,
`2024.3.3`, `2026.8.2`). Tudo nesta seção é **[documentação]**, salvo o que diz "a conferir": é o que
a documentação não deixa claro, e o hub foi escrito na leitura mais segura. O motor falso do kit de
teste (`arauto-testkit`, módulo `engine`) fala este formato; a primeira rodada do teste de contrato
contra um motor de verdade é o que transforma esta seção em fato **[observado]**.

`{s}` é o nome da sessão do motor. `@` dentro de um identificador vai no caminho como `%40`
("Remember to escape `@` in `chatId` and `messageId` with `%40`").

### Chave da API e servidor

| Formato | Origem e observação |
|---|---|
| Cabeçalho `X-Api-Key` em toda chamada | [documentação] Sem chave ou com a errada: 401, `{"message":"Unauthorized","statusCode":401}`. Nenhum endereço fica fora da chave por padrão. O hub trata 401 e 403 como "chave recusada" e não lê o corpo. |
| `GET /health` → `{"status", "info", "error", "details"}`, `status` ∈ `ok`, `error`, `shutting_down` | [documentação] O hub só aceita `ok`. |
| `GET /api/server/version` → `{"version", "engine", "tier", "browser"}` | [documentação] O hub lê `version`, `engine` e `tier` e compara (`version`, `engine`) com a lista das versões testadas, hoje só (`2026.10.1`, `GOWS`). |

### Sessões do motor e pareamento

| Formato | Origem e observação |
|---|---|
| `GET /api/sessions/{s}` → `{"name", "status", "config", "me", "engine"}`, com `me` = `{"id", "pushName"}` ou nulo | [documentação] A resposta para uma sessão que não existe: a conferir. O hub lê 404 como "não existe". |
| `POST /api/sessions` com `{"name", "start": false, "config"}` | [documentação] `start: false` adia a partida: o hub cria a sessão já configurada e só a liga ao parear. |
| `PUT /api/sessions/{s}` com a configuração inteira | [documentação] O corpo é uma configuração nova inteira; sessão que não está parada é parada e ligada de novo. Confirma a primeira armadilha. |
| `POST /api/sessions/{s}/start`, `/stop`, `/restart`, `/logout` | [documentação] `start` e `stop` são idempotentes; `stop` não sai do WhatsApp; `logout` sai e guarda a configuração. |
| `DELETE /api/sessions/{s}` | [documentação] Sai do WhatsApp e apaga configuração e dados. O hub chama `logout` e depois `DELETE`, e só com a palavra explícita do admin. |
| Estados: `STOPPED`, `STARTING`, `SCAN_QR_CODE`, `WORKING`, `FAILED`, `PASSKEY_REQUIRED`, `PASSKEY_CONFIRMATION_REQUIRED` | [documentação] Estado desconhecido não é adivinhado: a resposta conta como malformada. |
| `GET /api/{s}/auth/qr?format=raw` → `{"value"}` | [documentação] O valor vira QR no terminal da máquina do hub e não é guardado. |
| `POST /api/{s}/auth/request-code` com `{"phoneNumber"}` → `{"code"}` | [documentação] |
| Restrição do nome da sessão | A conferir: a documentação só diz que o nome funciona como identificador. O hub sorteia o alfabeto mais estreito, `^[a-z][a-z0-9]{15,62}$`. |

### A configuração de cada sessão do motor

| Campo | Origem e o que o hub põe |
|---|---|
| `webhooks[]`: `url`, `events`, `hmac.key`, `retries` (`policy`, `delaySeconds`, `attempts`), `customHeaders` (`[{name, value}]`) | [documentação] Um destino só: a caixa de entrada do hub. Eventos `session.status`, `message.any`, `message.reaction`, `message.edited` e `message.revoked`. Reenvio `exponential`, 2 s, 7 tentativas. Nenhum cabeçalho extra. A página não dá os valores padrão do reenvio. |
| `ignore`: `status`, `groups`, `channels`, `broadcast` | [documentação] `status`, `channels` e `broadcast` ligados; `groups` desligado, porque o histórico de grupo só vem no pareamento. |
| `client`: `deviceName`, `browserName` | [documentação] `deviceName` = `Arauto`. |
| `gows.storage`: `messages`, `groups`, `chats`, `labels`, `contacts`, `messageSecrets` | [documentação] Os cinco primeiros ligados. `messageSecrets` não é enviado: a conferir o padrão. |
| `metadata`, `proxy` (`server`, `username`, `password`), `debug` | [documentação] `{}`, sem proxy, `false`. Um proxy posto por fora é corrigido. |
| A chave de assinatura aparece de volta na leitura da sessão? | A conferir. Quando aparece, o hub a compara em tempo constante; quando não aparece, confia no resumo do que ele mesmo aplicou por último. |

### Eventos (a caixa de entrada do hub)

| Formato | Origem e observação |
|---|---|
| `X-Webhook-Hmac`: HMAC do corpo cru, com a chave configurada; `X-Webhook-Hmac-Algorithm`: `sha512` | [documentação] A codificação da assinatura não está escrita; o exemplo parece hexadecimal. O hub aceita só hexadecimal de 128 caracteres e recusa outro algoritmo nomeado. A chave configurada é o texto hexadecimal de 32 bytes sorteados; a conferir se o motor a usa como texto, como o hub supõe. |
| `X-Webhook-Request-Id`, `X-Webhook-Timestamp` (milissegundos) | [documentação] O hub não depende deles: a hora vem do envelope, que é o que a assinatura cobre. |
| Envelope: `id` (`evt_` e um ULID em minúsculas), `timestamp` (milissegundos), `event`, `session`, `metadata`, `me` (`id`, `pushName`), `payload`, `engine`, `environment` (`version`, `engine`, `tier`) | [documentação] O hub exige `id`, `timestamp`, `event`, `session` e `payload`; o resto é opcional. |
| `session.status`: `payload.status` e `payload.statuses` (`[{status, timestamp}]`) | [documentação] O telefone conectado vem do `me.id` do envelope. |
| `message` e `message.any`: `id`, `timestamp` (segundos), `from`, `fromMe`, `source`, `to`, `body`, `hasMedia`, `media` (`url`, `mimetype`, `filename`, `error`), `ack`, `vCards`, `_data` | [documentação] `participant` (o autor num grupo) e `replyTo` não estão no exemplo do evento: a conferir. `replyTo` aparece no exemplo da leitura de mensagens. |
| `message.reaction`: `id`, `from`, `fromMe`, `participant`, `to`, `timestamp`, `reaction` (`text`, `messageId`); `text` vazio tira a reação | [documentação] |
| `message.edited`: `id`, `editedMessageId` (sem a conversa), `body`, `_data` | [documentação] |
| `message.revoked`: `after` (`id`, `_data`), `revokedMessageId` (sem a conversa), `before` (quase sempre nulo) | [documentação] |
| Reenvio: `constant`, `linear`, `exponential` (com 20% de variação) | [documentação] A página não diz quando desiste; o "cerca de 2 minutos" das armadilhas é [observado] na versão conferida. |

### Leitura

| Formato | Origem e observação |
|---|---|
| `GET /api/{s}/chats/overview?limit&offset` → `[{"id", "name", "picture", "lastMessage", "_chat"}]` | [documentação] A ordem padrão não está escrita. |
| `GET /api/{s}/chats/{chat}/messages?limit&offset&downloadMedia=false&filter.timestamp.gte&filter.timestamp.lte` | [documentação] Os filtros de tempo estão em segundos (o exemplo é `1727745026`). A ordem padrão não está escrita: o hub ordena da mais nova para a mais antiga. |
| Uma mensagem lida: `id`, `timestamp`, `from`, `fromMe`, `body`, `hasMedia`, `ack`, `ackName`, `replyTo`, `_data` | [documentação] |
| Identificador de mensagem: `false_{conversa}_{id}`, e num grupo `false_{grupo}_{id}_{autor}` | [documentação] O `true_` (enviada pelo número) não aparece nos exemplos de mensagem, só no nome de um arquivo: a conferir. O hub exige a forma completa para editar e apagar. |
| `GET /api/{s}/chats/{chat}/messages/{id}?downloadMedia=true` → `media.url` = `.../api/files/{arquivo}`, baixado com `X-Api-Key` | [documentação] O exemplo é `/api/files/true_..._....jpg`, com `@` no nome. O hub nunca segue o endereço: pega só o caminho, sob `/api/files/`, num alfabeto estreito, e o pede ao motor que ele conhece, com a chave. |
| `GET /api/contacts?contactId&session` → `id`, `name`, `pushname`, `shortName`, `isMyContact` | [documentação] Não traz "conta comercial": o hub devolve esse dado como desconhecido. |
| `GET /api/contacts/about?contactId&session` → `{"about"}`; `GET /api/contacts/profile-picture?contactId&session` → `{"profilePictureURL"}` | [documentação] |
| `GET /api/{s}/groups?limit&offset` | [documentação] "Depende do mecanismo interno": o hub aceita lista ou objeto por identificador. A contagem de participantes é a da lista (armadilha). |
| `GET /api/{s}/groups/{grupo}`; `GET /api/{s}/groups/{grupo}/participants/v2` → `[{"id", "role"}]`, papéis `left`, `participant`, `admin`, `superadmin` | [documentação] O formato da resposta do grupo (assunto, descrição) não está na página: a conferir. Participante pode vir como `@lid`. |

### Envio e mudança

| Formato | Origem e observação |
|---|---|
| `GET /api/contacts/check-exists?phone&session` → `{"numberExists", "chatId", "pn"}` | [documentação] A página diz que números brasileiros anteriores a 2012 não têm o nono dígito e manda conferir antes de escrever: o hub tenta as duas formas. |
| `POST /api/sendText`: `session`, `chatId`, `text`, `reply_to`, `mentions`, `linkPreview` | [documentação] A menção também precisa estar no texto (`@número`): isso fica com quem escreve. A resposta (o identificador da mensagem enviada) não está na página: a conferir; sem identificador, o hub devolve "enviado" sem ele. |
| `POST /api/sendImage`, `/api/sendFile`, `/api/sendVoice`: `file` (`mimetype`, `filename`, `data` em base64), `caption`; voz com `convert` | [documentação] Só por conteúdo no MVP; o hub nunca manda `file.url`. |
| `POST /api/sendSeen`: `session`, `chatId`; sem `messageIds`, marca todas as não lidas | [documentação] |
| `POST /api/startTyping`, `POST /api/stopTyping` | [documentação] O corpo não está na página: a conferir. O hub manda `session` e `chatId`. |
| `PUT /api/reaction`: `session`, `messageId`, `reaction` | [documentação] "Reaction API uses PUT, not POST"; a tabela da mesma página diz POST. O hub usa PUT. |
| `POST /api/forwardMessage`: `session`, `chatId`, `messageId` | [documentação] Pode falhar para conversa que nunca recebeu mensagem. |
| `PUT /api/{s}/chats/{chat}/messages/{id}`: `text`, `linkPreview` | [documentação] O perigo de editar sem texto está nas armadilhas ([código]); a página não fala dele. |
| `DELETE /api/{s}/chats/{chat}/messages/{id}` | [documentação] |
| Limites do WhatsApp na sessão (`me.messageCapping`, `me.reachoutTimelock`) | A conferir: os nomes vêm do [código] da versão conferida, não da documentação. O hub só diz se existem. Um 429 do motor é lido como limite do WhatsApp. |

### Teste de contrato

Fica em `crates/arauto-hub/tests/whatsapp_contract.rs`, marcado `#[ignore]`: nunca roda na suíte
normal. Ele só fala com um motor e um número de teste, e só lê, a não ser que o envio seja pedido.

```
cargo test -p arauto-hub --test whatsapp_contract -- --ignored --nocapture
```

| Variável | Para quê |
|---|---|
| `ARAUTO_CONTRACT_ENGINE_URL` | O endereço do motor de teste, `http://host:porta`. |
| `ARAUTO_CONTRACT_ENGINE_KEY` | A chave da API dele. |
| `ARAUTO_CONTRACT_SESSION` | Uma sessão do motor já pareada com o número de teste. |
| `ARAUTO_CONTRACT_PHONE` | O telefone de teste dessa sessão, para a conferência das duas formas. |
| `ARAUTO_CONTRACT_SEND_TO` | Opcional: o telefone de teste que recebe um texto e uma reação. Sem ela, nada é enviado. |

Ele confere a saúde e a versão, a sessão, as conversas, as mensagens de uma conversa, os grupos com
os participantes, um contato e a conferência do telefone, e, com o envio pedido, o texto e a reação.
O formato do evento de mensagem ainda se confere à mão, olhando o que chega à caixa de entrada.
