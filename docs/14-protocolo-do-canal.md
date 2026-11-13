# 14 — Protocolo do canal

**Estado: proposta**, a validar no primeiro MVP ([15-mvp.md](15-mvp.md)). Revisada em 10/11/2026,
depois da revisão do desenho. É o formato do que passa entre o conector e o hub: a parte que
[04-seguranca-conexao.md](04-seguranca-conexao.md), seção 7, deixava para o protótipo.

Este documento não muda nenhuma regra do [04](04-seguranca-conexao.md). Onde os dois divergirem, vale
o 04, e o erro é daqui. O que a revisão achou e que pede mudança no 04 está na seção 12.

**O lado do hub já existe no código** (etapas 2 e 2b do MVP, 11/11/2026). Onde este texto era omisso,
o código escolheu uma leitura, e ela está escrita aqui com a marca "no hub". É fato só no sentido de
que a suíte do hub a confere, em Windows, contra um PostgreSQL de teste e um conector de teste: o
conector de verdade ainda não existe, e nada foi executado em Linux. Os pontos em que o código e o
texto ainda não se acertaram estão na seção 13.

## Resumo

```
 websocket (binário)
   └─ Noise XK            aperto de mãos em 3 mensagens; depois, quadros cifrados
        └─ quadro         1 byte de marcas + 4 bytes do fluxo + pedaço
             └─ mensagem  JSON com o campo "t" (o tipo)
```

| Camada | Escolha | Por quê |
|---|---|---|
| Transporte | Websocket, só mensagens binárias, sem compressão | Passa por proxy comum. O conteúdo já vai cifrado: comprimir não rende e abre um canal lateral. |
| Cifra | Noise `XK_25519_ChaChaPoly_BLAKE2s`, prólogo `arauto/1` | Decisão 0012. |
| Mensagem | JSON em UTF-8 | As chamadas e respostas das ferramentas já nascem e terminam em JSON. |

Três regras atravessam o documento:

1. **Antes da sessão, quase nada é aceito.** Quem ainda não provou que é um aparelho ativo só manda
   mensagens pequenas, uma de cada vez, com prazo.
2. **Só a conexão mais nova de um aparelho é julgada.** Uma tentativa abandonada que chega atrasada
   pela rede nunca decide nada.
3. **Tamanho nunca derruba quem recebe.** Quem envia confere antes; o que não cabe vira erro da
   chamada, não fechamento da conexão.

## 1. Endereços do hub

Todos na porta dos conectores.

| Endereço | Para quê |
|---|---|
| `GET /v1/channel` | O websocket. Serve à inscrição, ao canal e ao pedido de encerramento. |
| `GET /v1/download/{arquivo}` | O conector, numa versão exata. Público e sem segredo. Só serve nomes de uma lista fixa, montada na partida. |

- Requisição de leitura não muda estado: abrir qualquer um deles não gasta convite.
- **Não existe endereço que entregue a chave do hub.** O conector só aceita a chave que veio no
  bloco, e só descobre as versões pelo `hello`. Quem instala lê a chave pública no comando de
  administração.
- O conector valida o certificado TLS do endereço pelas autoridades públicas. Endereço `ws://`, sem
  TLS, só é aceito para `127.0.0.1` e `localhost`, em desenvolvimento. Autoridade privada fica para
  depois do MVP.
- Saúde e métricas ficam em outra porta, que não é publicada ([15](15-mvp.md), seção 6). No hub, a
  porta interna responde só `GET /health`, com `ok`; a de conectores não responde a ela.
- No hub, todo outro endereço, e todo método que não seja `GET`, recebe 404. Isso vale para a
  porta dos conectores e para a interna. A porta do painel, quando ligada, é uma terceira: os
  endereços dela não existem nessas duas, nem os delas na do painel
  ([decisão 0040](decisoes/0040-painel-dentro-do-hub.md)).
- **Download, no hub.** O nome pedido só é comparado com os nomes da lista e nunca vira caminho de
  arquivo: `../`, barra, caractere codificado ou letra trocada de caixa simplesmente não acham nada.
  Um nome da lista tem até 128 letras, dígitos, pontos, hífens e sublinhados, e não começa por ponto.
  No M1 a lista é vazia: todo download é 404.

Regras do websocket:

- Só mensagem binária. Mensagem de texto fecha a conexão.
- Cada mensagem tem no máximo 65.535 bytes. O hub configura esse teto na própria biblioteca: uma
  mensagem maior nem chega a ser montada.
- A extensão de compressão não é negociada.
- O hub dá 5 segundos para os cabeçalhos da requisição chegarem. No hub, o prazo vale também entre
  duas requisições da mesma conexão: a conexão HTTP fica aberta entre elas, porque fechá-la quebraria
  a troca para websocket na biblioteca usada.
- Os `ping` do próprio websocket são respondidos pela biblioteca e não contam como sinal de vida. O
  batimento que conta é o do protocolo (seção 4).

## 2. Aperto de mãos

O conector é quem inicia. A chave estática dele é a identidade do aparelho; a do hub veio no bloco.

```
 conector -> hub   e, es            48 bytes
 hub -> conector   e, ee            48 bytes
 conector -> hub   s, se            64 bytes
```

- As três mensagens vão sem carga e têm tamanho exato. Outro tamanho fecha a conexão.
- O prólogo é `arauto/1` e **não muda** com a versão do protocolo.
- Um hub que não tem a chave do bloco não conclui: o conector fecha com AR-202.
- Uma chave que não é de ninguém (um ponto de ordem baixa, com o qual toda conta dá zero) é recusada
  dos dois lados: nem identidade de aparelho, nem chave de hub.
- O hub dá 10 segundos, contados da abertura do websocket, para o aperto de mãos terminar. Passou
  disso, fecha sem responder.
- O **resumo do aperto de mãos** é o valor `h` do Noise ao fim da terceira mensagem (32 bytes). Os
  dois lados o leem antes de passar aos quadros. Ele entra no código de conferência (seção 8).
- O padrão XK não barra identidade desconhecida: qualquer um que tenha a chave pública do hub conclui
  o aperto de mãos. Por isso os limites da seção 4 valem para toda conexão.

## 3. Quadros

Depois do aperto de mãos, cada mensagem binária do websocket é uma mensagem Noise de transporte.

- Cada sentido tem um contador, que começa em zero e sobe um a cada quadro. O contador **não viaja**:
  quem recebe usa o próprio. Um quadro repetido, faltando ou fora de ordem não decifra, e a conexão é
  fechada.
- **Só a tarefa que escreve no websocket cifra**, na hora de escrever, depois de escolher entre a fila
  de controle e a de dados. Cifrar ao enfileirar quebraria o contador no primeiro quadro que passasse
  na frente.

O conteúdo decifrado de um quadro:

```
 byte 0      marcas: bit 0 = FIM (último pedaço da mensagem); bit 1 = CONTROLE
 bytes 1-4   número do fluxo, sem sinal, do byte mais significativo para o menos
 bytes 5...  pedaço da mensagem
```

| Regra | Detalhe |
|---|---|
| Bits 2 a 7 | Sempre zero. Outro valor fecha a conexão. |
| Mensagem de um quadro só | Vai no **fluxo 0**, com FIM. Não conta como fluxo aberto. |
| Mensagem maior | É cortada em pedaços de até 60.000 bytes, todos com o mesmo número de fluxo; o último leva FIM. |
| Pedaço vazio | Erro: fecha a conexão. |
| Numeração | O conector usa os ímpares e o hub os pares a partir de 2, sempre crescendo. Número de paridade errada, repetido ou menor que o último aberto por aquele lado fecha a conexão. |
| Intercalação | Fluxos diferentes podem se intercalar. A mensagem é entregue quando chega o quadro com FIM. |
| CONTROLE | Batimento e conferência. Sempre um quadro só, no fluxo 0, com FIM. Passa na frente da fila de dados. |
| Quem envia | Só abre um fluxo com a mensagem inteira pronta, e espera lugar se já tem 8 abertos. Nunca estoura o limite de propósito. |
| Fim dos números | Quando os números de fluxo de um lado acabam, ele fecha a conexão com AR-205 e reconecta. |

Uma mensagem é interpretada uma vez só. Nenhum lado repassa adiante os bytes como recebeu: o hub
monta o que manda ao motor a partir dos argumentos já validados.

## 4. Limites e prazos

Os limites existem porque a conexão é aberta a qualquer um até a sessão começar.

| Fase | O que vale |
|---|---|
| Websocket | Mensagem de até 65.535 bytes; só binária. |
| Aperto de mãos | Tamanhos exatos (48, 48, 64); 10 segundos. |
| Antes de `session` | Só o fluxo 0. Mensagem de até 4.096 bytes. O `hello` em 5 segundos. Cada resposta do conector em 10 segundos; o `saved`, em 60. Fora a espera do pedido, a conexão inteira tem 90 segundos para virar sessão. |
| Em espera | O aparelho pendente só manda `check_ok`, `ping`, `pong` e `leave`. |
| Em sessão | Do conector para o hub, mensagem de até 6 MiB. Do hub para o conector, até 16 MiB. Oito fluxos abertos **por sentido**, contados por quem recebe. Um fluxo aberto que fica 60 segundos sem pedaço novo fecha a conexão. |
| Chamadas | Até 32 em andamento por conexão. A seguinte é recusada com AR-710, sem fechar. |
| Conteúdo de uma mensagem | No máximo 100.000 valores JSON e 64 níveis de aninhamento. Sem isso, uma mensagem de poucos MiB feita só de zeros custaria dezenas de vezes o tamanho em memória. Quem envia confere antes: vira AR-709 da chamada. |
| O hub inteiro | Um orçamento de bytes para mensagens em remontagem (256 MiB de partida). Sem espaço, a conexão que pede mais é fechada com AR-205. |
| Conexões sem sessão | No máximo 1.024 no hub e 16 por origem, fora as que estão em espera. Acima disso o hub responde 503 ou 429 antes de abrir o websocket. |

Como o hub aplica a tabela:

- **Os 90 segundos** contam desde a abertura do websocket, com o aperto de mãos dentro, e param
  enquanto a conexão está em espera.
- **A origem** é o endereço IPv4, ou os primeiros 64 bits do endereço IPv6 (uma rede de uma casa ou
  de uma operadora). Um IPv4 escrito como IPv6 vale como o IPv4. De onde vem o endereço está em
  [15](15-mvp.md), seção 4.
- **Fora do teto**: a conexão em espera e a que está em sessão. Uma conexão TCP que ainda não mandou
  os cabeçalhos também não conta: só o prazo de 5 segundos a limita.
- **Mensagem que só o hub envia**, vinda do conector, fecha a conexão, antes ou depois da sessão.
- **O orçamento do hub inteiro**: o espaço que uma conexão tomou dele volta quando ela termina.
- **Mensagens grandes**: acima de 256 KiB, o hub interpreta o JSON fora das tarefas de rede.
- **Escrita.** Cada conexão tem duas filas de saída: controle, com 16 quadros, e dados, com 64. Quem
  produz dado espera lugar. A fila de controle cheia fecha a conexão sem `closing`. Um quadro que não
  sai em 30 segundos também fecha: o prazo é por quadro, e não "nenhum byte andou em 30 segundos".
- **O código.** Silêncio de 90 segundos, `hello` fora do prazo, resposta fora do prazo e `ping` ou
  `pong` antes do `hello_ok` fecham com AR-204.

O que não cabe:

- Quem monta uma mensagem confere o tamanho antes de enviar.
- Uma resposta que não cabe vira `result` com `ok: false` e AR-709. As leituras têm orçamento em
  bytes e cortam a lista em itens inteiros ([09](09-contrato-mcp.md), [13](13-conversa-entre-hermes.md)),
  então o AR-709 só aparece num item sozinho grande demais.
- O conector recusa localmente, com AR-709, a chamada cujos argumentos não cabem.

Prazos do lado do conector, sempre maiores do que a soma dos do hub:

| Espera | Prazo |
|---|---|
| `hello_ok` e `state` | 10 segundos cada |
| `check`, depois do `present` | 15 segundos |
| `credential`, depois do `check_ok` | 20 segundos |
| `session`, depois do `saved` | 20 segundos |
| `enrolled` | 15 segundos |
| `left` | 10 segundos |

**O conector encerra uma tentativa antes de abrir outra** e, depois de fechar uma tentativa em que já
mandou `present` ou `enroll`, espera 6 segundos para abrir a seguinte com a mesma identidade. O `join`
segura uma trava da pasta enquanto roda: dois ao mesmo tempo não acontecem.

Batimento: cada lado manda `ping` a cada 30 segundos, desde o `hello_ok`, inclusive na espera.
Qualquer quadro válido recebido conta como sinal de vida. Noventa segundos sem receber nada encerram
a conexão, com AR-204.

Reconexão: depois de uma queda, a primeira tentativa é na hora (C9). As seguintes esperam um tempo
sorteado entre zero e o teto, que dobra a cada falha, de 1 a 60 segundos. Sem o sorteio, os
conectores recusados numa volta em massa tentariam de novo todos juntos.

Todo prazo desta seção vem de uma configuração e de um relógio que os testes controlam.

## 5. Mensagens

Toda mensagem é um objeto JSON com `"t"`. Campo desconhecido é ignorado. Chave repetida é erro.
Dados binários vão em base64 padrão, com preenchimento.

Mensagem de tipo desconhecido: antes da sessão, fecha a conexão; depois, é ignorada, e a centésima
fecha com AR-204.

### 5.1 A primeira mensagem, e o que nunca muda

A primeira mensagem é do conector:

```json
{"t":"hello","proto":[1,1],"connector":"0.1.0","purpose":"channel"}
```

| Campo | Conteúdo |
|---|---|
| `proto` | A menor e a maior versão de protocolo que o conector fala. |
| `connector` | A versão do programa. É texto declarado: aparece na tela e não decide nada. |
| `purpose` | `enroll` (inscrição), `channel` (canal) ou `leave` (pedido de encerramento). Outro valor: `closing` com AR-204. |

O hub responde `hello_ok`:

| Caso | Resposta |
|---|---|
| Há versão em comum | `{"t":"hello_ok","proto":N}`, com a **maior** versão comum. |
| O conector é antigo demais | Em `channel`: `{"t":"hello_ok","proto":0,"outdated":true}` e segue a apresentação sem troca (5.3). Em `enroll`: `closing` com AR-401. Em `leave`: atende. |
| O conector é novo demais | `closing` com AR-402, em qualquer `purpose`, inclusive `leave`. |

No hub, o `leave` de um conector novo demais também recebe AR-402: não existe `hello_ok` válido para
uma faixa de versões toda acima da do hub. O C8 diz que o `leave` é aceito "em qualquer versão"; a
leitura do hub é que a exceção vale para o conector antigo demais (seção 12).

Quem decide o AR-401 é a versão do **protocolo**. A lista de versões do programa, com revogação,
fica para depois do MVP.

**Congelado para sempre**, para um conector de qualquer idade ainda receber o erro certo: o aperto de
mãos e o prólogo, o cabeçalho do quadro, e as mensagens `hello`, `hello_ok`, `state`, `present`,
`check`, `check_ok`, `closing`, `leave` e `left`.

### 5.2 Inscrição (`purpose: enroll`)

| De | Mensagem | Campos |
|---|---|---|
| conector | `enroll` | `invite`; `declared`, com `hostname`, `os` e `hermes_version` (texto não confiável, de uma linha, até 128 caracteres cada, sem caractere de controle nem de direção) |
| hub | `enrolled` | `nonce` (32 bytes), `confirm_within_s`, `enrollment` (de 1 a 5) |
| hub | `closing` | `code`: AR-101, AR-102, AR-104, AR-301, AR-302 ou AR-303 |

- O `enroll` só é mandado depois do `hello_ok`. Antes disso, ou repetido, fecha a conexão.
- Depois de `enrolled`, o conector calcula o código, mostra e fecha. É uma conexão curta.
- As recusas de inscrição são registradas **agregadas**: uma linha por origem, organização, código e
  minuto, com a contagem. Uma linha por tentativa deixaria qualquer um encher uma tabela que não se
  apaga. No hub vale para toda recusa, e não só para o AR-101 de quem não tem aparelho: o AR-102
  repetido também é contado assim. A contagem fica em memória até o minuto fechar, e a varredura a
  grava. **Limite:** o que foi contado no minuto em que o hub para se perde.
- O convite sem organização (resumo que ninguém emitiu) recebe AR-101 e entra na contagem sem
  organização.
- No hub, um convite vencido que a inscrição encontra é encerrado ali mesmo, com o evento
  `invite.expired`, sem esperar a varredura.
- Um convite ainda não usado, apresentado por uma identidade que já tem linha de aparelho, de
  qualquer Hermes, recebe AR-101 e não é gasto: identidade velha não abre pedido novo.
- Pedido já resolvido responde AR-104 em todo estado que só existe depois de uma aprovação: ativo,
  quarentena, revogado, substituído e vencido por inatividade.

### 5.3 Canal (`purpose: channel`)

Depois do `hello_ok` **o hub sempre fala primeiro**, com `state`.

| `state` | Quando | O que segue |
|---|---|---|
| `waiting` | Aparelho pendente. | O hub manda `check`. Respondido, a conexão fica **em espera**. Quando o pedido é resolvido, o hub manda outro `state` ou um `closing`. |
| `approved` | Aparelho aprovado, na geração zero. | A troca (seção 6). |
| `active` | Aparelho ativo. | A troca (seção 6). |
| `ended`, com `code` | Qualquer outro estado, identidade desconhecida, ou conector antigo demais. | A **apresentação sem troca**: o conector apresenta o que tem, responde à conferência e recebe `closing` com o código. O hub nunca emite credencial aqui. |

A apresentação sem troca existe porque o 04 manda olhar o que um aparelho encerrado apresenta (C7:
"reuso depois do encerramento"; C11: lacre que o banco não explica) e porque a decisão 0030 manda
conferir a credencial do conector antigo antes de recusar a versão. Um conector antigo demais se
apresenta assim na partida do líder e, depois, uma vez a cada 24 horas.

| De | Mensagem | Campos | Quando |
|---|---|---|---|
| hub | `state` | `state`, e `code` em `ended` | Sempre a primeira depois do `hello_ok`; de novo quando a espera termina. |
| conector | `present` | `credential` (até 1.024 bytes) ou `null` | Em `approved`, `active` e `ended`. `null` quando não tem credencial. Fora dessa ordem, fecha. |
| hub | `check` | `nonce` (16 bytes) | A conferência da regra 3 do C7. Marca CONTROLE. |
| conector | `check_ok` | `nonce` | A mesma. Marca CONTROLE. |
| hub | `credential` | `generation`, `sealed` | A credencial nova, já gravada como pendente. |
| conector | `saved` | `generation`, `digest` (SHA-256 do bloco recebido) | O "gravei", depois de a gravação estar garantida em disco. |
| hub | `session` | `hermes` (nome), `tools` (nomes liberados), `wake` (pode ser acordado) | A promoção foi gravada. A sessão do canal começa aqui. |
| hub | `tools` | `tools` | A lista de ferramentas liberadas mudou. |
| conector | `call` | `id`, `tool`, `args` | Chamada de ferramenta. |
| hub | `result` | `id`, `ok`, e `content` ou `code`; `retry_after_s` opcional | Resposta. Recusa leva só o código; a frase fica no conector. |
| hub | `notice` | `id`, `kind`, e os campos do tipo | Aviso: ponteiro, sem texto de terceiros. |
| conector | `notice_ack` | `id`, `outcome` | Depois de a API local do Hermes aceitar. |
| hub | `rotate` | | Começa a troca de 24 horas. |
| hub | `rotated` | `generation` | A troca de 24 horas foi gravada. |
| os dois | `ping`, `pong` | `n` | Batimento. O `pong` devolve o `n`. Marca CONTROLE. |
| conector | `leave` | | Pedido de encerramento pelo canal aberto, em espera ou em sessão. |
| hub | `left` | `status` | Resposta ao `leave`. |
| hub | `closing` | `code` | O motivo do fechamento (seção 7). |

**Chamadas.**

- `call` antes de `session` é erro de protocolo: a conexão é fechada. É a regra 4 do C7.
- O `id` é um inteiro positivo, do conector, único entre as chamadas em andamento daquela conexão.
  Repetido, fecha a conexão.
- O hub valida de novo o nome da ferramenta e todos os argumentos: o conector pode ser hostil.
  Ferramenta que não existe ou argumento malformado: AR-711. Ferramenta não liberada: AR-701.
- `retry_after_s` é um inteiro de 1 a 86.400. O conector o valida e o encaixa na frase fixa do erro.
- Uma chamada que não chegou a sair quando o canal cai volta à LLM com AR-203: pode repetir. Uma
  chamada que saiu e ficou sem resposta volta com AR-207: o resultado é desconhecido, e a frase manda
  conferir antes de repetir. O hub não reconhece chamada repetida.

**Como o hub atende a sessão:**

- **Mensagem mandada antes de ler o `session`.** Toda mensagem que o conector mandou antes de o
  `session` sair do hub (um `call`, um `leave`, qualquer outra) fecha a conexão com AR-204, depois
  do `session`. O hub sabe a ordem exata: a conexão passa à sessão no instante em que o `session`
  sai da fila de envio. A promoção já está gravada, e o conector volta com a credencial nova.
- **A ordem de cada chamada:** primeiro o hub relê o estado do aparelho (C8). Se ele não serve
  mais, a resposta é `result` com o código do estado (por exemplo, AR-502), seguido de `closing`
  com o mesmo código. Só depois vêm o nome e os argumentos (AR-711), a liberação (AR-701) e o
  tamanho da resposta (AR-709).
- **O identificador repetido** fecha com AR-204. Um identificador sai das chamadas em andamento
  antes de o `result` dela ser mandado.
- **A 33ª chamada em andamento** recebe AR-710, mandado pelo próprio laço da sessão: um conector
  que insiste espera as recusas saírem, e a espera chega à conexão dele. Essa recusa **não relê o
  estado do aparelho** (seção 12).
- **No M1** só `arauto_status` é liberada, e o hub a responde sem o motor. Os campos estão em
  [09](09-contrato-mcp.md), seção 3. As `hermes_*` existem no catálogo e recebem AR-701; as
  `whatsapp_*` ainda não estão no catálogo e recebem AR-711. A mensagem `tools` nunca é mandada,
  porque a lista não muda, e nenhum `notice` sai; um `notice_ack` é ignorado.
- **`saved` fora da troca de 24 horas** fecha a sessão com AR-204 (C7, regra 7).
- **O `leave` pelo canal aberto** recebe `left` e a conexão termina.

**Avisos.**

| `kind` | Campos | Validação no conector, antes de montar a frase fixa |
|---|---|---|
| `whatsapp` | `number` (apelido), `chat` (identificador opaco), `count`; o `id` de todo aviso é um UUID | `number` no formato de nome; `chat` com até 64 caracteres de `A-Z a-z 0-9 _ -`; `count` de 1 a 9.999 |
| `peer` | `from` (nome do Hermes), `count` | `from` no formato de nome; `count` de 1 a 9.999 |

O formato de nome é `^[a-z0-9][a-z0-9_-]{0,31}$`, e vale para o nome do Hermes e para o apelido do
número. A validação fica **compilada no conector**: mesmo um hub invadido só consegue pôr na frase
um nome nesse formato e dois números.

| `outcome` | Quando | O que o hub faz |
|---|---|---|
| `accepted` | A API local aceitou. | Dá o aviso por confirmado. |
| `duplicate` | O conector já tinha tratado aquele `id`. | Dá por confirmado. |
| `failed` | A API local não respondeu ou recusou. | Tenta de novo, com espera crescente, até o aviso expirar. |
| `throttled` | O conector atingiu o próprio teto de acordadas. | Tenta de novo em 5 minutos. |
| `rejected` | Tipo desconhecido ou campo fora do formato. | Descarta o aviso. Nunca tenta de novo. |

### 5.4 Pedido de encerramento

| De | Mensagem | Campos |
|---|---|---|
| conector | `leave` | |
| hub | `left` | `status`: `closed`, `already_closed`, `unknown` |

- Com o canal aberto, o pedido vai por ele (C10). Sem canal, vai por uma conexão curta com
  `purpose: leave`, uma por identidade. Conexão curta não apresenta credencial e não conta como
  conexão dupla.
- Os três valores valem como confirmação para o conector apagar a pasta daquela identidade.
- Um pedido pendente que pede o encerramento vira negado.
- No hub, um aparelho ativo que sai fica revogado, com o alerta de encerramento pedido pelo aparelho
  e o endereço de onde veio (C10).
- **Se o banco voltou de uma cópia** e o hub descobre isso no meio do `leave`, nada é gravado: o hub
  liga o modo restaurado (seção 6, "Modo restaurado com o hub no ar"), e a conexão curta fecha com
  AR-205, sem `left`.
- **Se a marca da auditoria não avançou** depois de o encerramento estar gravado, o hub grava o
  alerta `mark_not_advanced` para o admin e só então responde `left` com `closed`, porque o
  encerramento está no banco. O alerta vem antes para não se perder se o hub cair entre os dois:
  depois do `left`, o conector apaga a pasta.

## 6. A troca, passo a passo

O hub mantém, em memória, o registro das conexões de canal de cada aparelho. Cada conexão recebe um
**número de ordem** ao fim do aperto de mãos. Uma conexão está **estabelecida** quando está em espera
ou em sessão.

```
 1 hub       state
 2 conector  present                       (em espera não há present)
 3 hub       check        prazo: 5 s       sem resposta: fecha, sem consequência
 4 conector  check_ok
 5 hub       esta é a conexão mais nova do aparelho?   não -> fecha com AR-206, sem consequência
 6 hub       fecha com AR-206 as mais antigas que ainda não estão estabelecidas
 7 hub       pré-julgamento, só de leitura (tabela abaixo)
 8 hub       se a apresentação seria aceita e há uma conexão estabelecida:
             manda check a ela e espera 3 s
               respondeu      -> conexão dupla (C9; no pendente, C6)
               não respondeu  -> fecha a antiga com AR-208; a nova segue
 9 hub       T1: trava, relê, julga de novo, grava a pendente e a maior geração
10 hub       credential
11 conector  grava em disco e manda saved  prazo: 60 s
12 hub       T2: trava, relê, confere e grava a promoção
13 hub       session
```

- **Nenhuma transação fica aberta durante a rede.** As esperas dos passos 3, 8 e 11 acontecem sem
  trava e sem conexão do banco. T1 e T2 são transações curtas.
- **Toda gravação que muda um aparelho relê o estado antes.** T1 e T2 travam primeiro a linha do
  Hermes e depois as dos aparelhos dele, e só gravam se o aparelho continua aprovado ou ativo. Em T2,
  só se o resumo recebido no `saved` é o da pendente emitida **naquela conexão**. O comando de
  administração trava na mesma ordem. Assim um aparelho revogado entre o passo 10 e o 12 não volta a
  ativo.
- Os passos 5 e 6 fecham a corrida da tentativa abandonada: se a resposta de uma tentativa antiga
  chega depois de o conector já ter aberto outra, ela é ignorada.
- Um `saved` que não se refere à pendente daquela conexão fecha a conexão (C7, regra 7).
- Se o passo 8 fechou uma sessão que estava servindo, sai o alerta "a conexão de {nome} foi tomada
  por outra" (C9).
- Uma conexão que recebeu `credential` e terminou sem `saved` fica registrada. Três em 24 horas geram
  alerta (C7).

**Como o hub faz os passos:**

- **O registro** é um mapa repartido por identidade do aparelho, na memória de um processo. Cada
  conexão tem o seu número de ordem e o seu estado: tentativa, em espera ou em sessão.
- **Uma tentativa por vez.** Cada identidade tem a sua vez de ser julgada, do passo 5 ao fim de T1.
  Logo antes de T1 o hub confere de novo que aquela conexão continua a mais nova e aberta; se não,
  ela fecha com AR-206 e não recebe nada. Sem isso, uma tentativa abandonada que passou do passo 5
  podia rodar T1 depois de uma mais nova já ter trocado a credencial, e mandar o aparelho legítimo à
  quarentena por reuso. Foram as rodadas sorteadas que acharam esse caso.
- **O pré-julgamento** (passo 7) só lê. Se o banco falha nele, a conexão fecha com AR-205 e não
  chega a T1.
- **O teste do passo 8** é respondido pela tarefa que lê da conexão antiga: um `check_ok` com o
  nonce do teste não espera a sessão, que pode estar ocupada com o banco. Uma conexão antiga que já
  está terminando não é testada. Se a antiga responde, as duas recebem AR-501 e o aparelho vai à
  quarentena; no pendente, o pedido é negado (AR-301) e sai o alerta de identidade copiada. Se não
  responde, só a antiga recebe AR-208. O alerta de conexão tomada só sai se ela servia uma sessão no
  momento do teste e continuava aberta ao fim do prazo.
- **O `state` sai antes do `check`.** O `check` é de controle e passa na frente da fila de dados;
  por isso o hub espera o `state` sair antes de mandá-lo.
- **A prova de restauração** é olhada antes de qualquer prazo vencido, para o prazo não esconder a
  prova.
- **Na primeira confirmação**, o aparelho substituído (o antigo do mesmo Hermes) recebe AR-503, se
  tiver conexão aberta.
- **Promoção pela apresentação da pendente.** Um aparelho aprovado que volta apresentando a
  pendente (caiu depois de gravar, antes de o hub ouvir o "gravei") passa a ativo já em T1, e essa é
  a primeira confirmação. A sessão continua só depois do "gravei" da credencial seguinte (seção 12).
- **Falhas na gravação de segurança.** Se T1 ou T2 descobrem um banco restaurado, o hub liga o modo
  restaurado. Se a marca da auditoria não avançou depois de T2, a promoção fica gravada, mas o hub
  não abre a sessão: fecha a conexão e grava o alerta `mark_not_advanced`; na volta o conector
  apresenta o que tem, e ninguém fica trancado. Se a conexão já fechou quando T1 ou T2 rodam, nada é
  gravado.
- **Nenhuma espera de rede segura transação ou conexão do banco**, inclusive a espera do pendente.

**A espera do pendente.** O hub manda `state` `waiting` e `check`; respondido, a conexão fica em
espera e sai do teto de conexões sem sessão. O hub a cutuca quando chega um aviso do comando de
administração ou quando a varredura vê que o pedido saiu de pendente. Aprovado: novo `state` e a
troca. Negado ou vencido: `closing` com o código. Um `leave` é aceito durante a espera.

**Modo restaurado com o hub no ar.** Venha a prova de onde vier (a varredura, T1, a apresentação de
um desconhecido, um `leave`), o hub liga o modo e fecha as sessões e as conexões em espera com o
código do novo estado do aparelho (suspenso: AR-506). As demais conexões recebem AR-205.

**O julgamento**, para aparelho aprovado ou ativo (é a tabela do C7):

| O conector apresenta | O hub |
|---|---|
| A vigente, ou `null` na geração zero | Aceita. Emite uma nova, que passa a ser a pendente; a pendente anterior é descartada. |
| A pendente | Aceita. Promove a pendente a vigente, a vigente anterior morre, e emite outra. |
| Outra já emitida, ou `null` depois da geração zero | Quarentena e alerta: "reuso". |
| Um lacre que não abre, ou de outro aparelho | Quarentena e alerta: "credencial ilegível". |
| Um lacre válido de geração maior que todas as conhecidas | Prova de restauração: liga o modo restaurado (C11). |

Na apresentação sem troca o hub só olha, nunca emite:

| Situação | O hub |
|---|---|
| Conector antigo demais, aparelho aprovado ou ativo | Aplica as três últimas linhas da tabela acima. Depois, `closing` com AR-401. |
| Aparelho revogado, substituído ou vencido por inatividade, com lacre válido mais antigo do que a última vigente e a última pendente | Alerta "reuso depois do encerramento" (C7). Depois, o código do estado. |
| Qualquer estado, lacre válido de geração maior que todas as conhecidas | C11: prova de restauração, a não ser que o aparelho já esteja marcado. |
| Identidade desconhecida com lacre válido | C11: entra na lista de aparelhos desconhecidos e é prova de restauração, a não ser que já esteja listada. AR-504. No hub, com o modo restaurado ligado, ela é listada e não é prova (seção 13). |
| Identidade desconhecida com `null` ou com lacre que não abre | AR-504. Nada é gravado por tentativa. |

Mais quatro leituras do hub para esta tabela:

- O lacre do desconhecido é tentado com a chave de cada organização.
- A prova de um desconhecido fica guardada na linha da lista, como **prova pendente**, até o modo
  restaurado ser ligado. Um hub que para entre a listagem e o modo encontra a prova de novo na
  apresentação seguinte, e ligar o modo trata de uma vez todas as provas pendentes.
- Um aparelho aprovado ou ativo, já marcado por restauração, que apresenta um lacre de geração maior
  vai à quarentena por reuso; nos outros estados, isso só gera registro. Pelo C11 essa combinação
  nem deveria existir.
- O código de um aparelho vencido sem motivo que o explique é AR-507: é o único que nunca leva o
  conector a descartar nada.

**A troca de 24 horas.** O hub manda `rotate`, faz T1 como se a vigente tivesse sido apresentada,
manda `credential`, espera o `saved` por 60 segundos, faz T2 e responde `rotated`. As chamadas seguem
durante a troca. Sem o `saved` no prazo, o hub fecha a conexão, e na volta vale a tabela de sempre.
No hub, cada sessão confere a hora da troca no mesmo intervalo da varredura (2 segundos), contando
desde a última troca confirmada daquela conexão. O `saved` que não chega no prazo e o `saved` de
outra credencial fecham com AR-204.

## 7. Fechamentos

O hub manda `closing` com o código antes de fechar, sempre que consegue. No hub, o `closing` não
passa na frente da fila de dados: sai depois do que já estava na fila, para manter a ordem (por
exemplo, o `hello_ok` antes do AR-205), e nada do que for enfileirado depois dele sai. A escrita da
fila toda tem o mesmo prazo de 30 segundos. Só os quadros de controle passam na frente. Uma conexão
ainda no aperto de mãos, ou com a fila de controle cheia, fecha sem `closing`.

| Código | Quando | O que o conector faz |
|---|---|---|
| AR-204 | Erro de protocolo, limite estourado ou mensagem inválida. | Reconecta com espera crescente. Não repete a mesma mensagem. |
| AR-205 | O hub está parando ou sem recursos. | Reconecta depois de 1 a 10 segundos, sorteados. |
| AR-206 | Esta tentativa foi trocada por outra mais nova do mesmo aparelho, antes de virar canal. | Espera de 5 a 15 segundos, sorteados, e tenta de novo. |
| AR-208 | Dada por morta: não respondeu ao teste, e outra conexão do mesmo aparelho entrou no lugar. | Reconecta na hora (C9). Se alguém tomou o lugar dele, é aí que aparece. |
| AR-301, AR-302, AR-303 | O pedido foi negado ou venceu. | [10](10-maquinas-de-estado.md), seção 6: só descarta pasta sem credencial. |
| AR-401, AR-402 | Versão de protocolo sem acordo. | Para de usar o canal e mostra o erro em toda ferramenta. |
| AR-501, AR-506 | Quarentena; suspenso. | Para de usar o canal. Não apaga nada. |
| AR-502 | Revogado. | Para de usar o canal. Não apaga nada. |
| AR-503 | Substituído. | Passa a usar a pasta do aparelho novo, se houver. |
| AR-504 | O hub não reconhece o aparelho. | Para de usar o canal. Não apaga nada. |
| AR-507 | Vencido por inatividade. | Para de usar o canal. Não apaga nada. |

"Parar de usar o canal" não encerra o conector ([10](10-maquinas-de-estado.md), seção 6).

Parada em ordem do hub, na leitura que não perde resultado:

1. Deixa de aceitar conexões. Um pedido de websocket que ainda chegue recebe 503, e as conexões que
   ainda estão em HTTP ou no aperto de mãos são cortadas.
2. Manda `closing` com AR-205, na hora, a cada conexão que não está em sessão.
3. Cada sessão recusa chamada nova com AR-205, manda o `result` das chamadas em andamento e só então
   manda `closing` com AR-205.
4. Passados 8 segundos, fecha à força o que sobrou, com AR-205, e espera mais 2 segundos para o
   `closing` sair. A parada pode levar, então, até 10 segundos.

## 8. Código de conferência

```
 material = HKDF-SHA-256( sal   = nonce do hub (32 bytes),
                          chave = resumo do aperto de mãos (32 bytes),
                          info  = "arauto/code/1" ),  com 64 bytes de saída
 código   = seis dígitos tirados do material, sem viés
```

"Sem viés": o material é lido em 16 palavras de 4 bytes, do byte mais significativo para o menos. A
primeira palavra menor que 4.000.000.000 é reduzida módulo 1.000.000; as outras são descartadas. Se
nenhuma servir, o hub sorteia outro nonce antes de mandar o `enrolled`. O código é mostrado como
`000-000`.

O nonce só sai do hub depois da terceira mensagem do aperto de mãos, pelo canal já cifrado (C4).

Vetores de teste, para duas implementações baterem sem depender da mesma biblioteca:

| Nonce (hex) | Resumo (hex) | Código |
|---|---|---|
| `00` × 32 | `00` × 32 | `028-213` |
| `000102…1f` | `202122…3f` | `935-415` |

A suíte de `crates/arauto-protocol` guarda os dois e os confere com uma segunda implementação.

## 9. O lacre

A credencial lacrada é um bloco de bytes que só o hub abre.

```
 lacre       = versão (1 byte, valor 1) || nonce (24 bytes) || XChaCha20-Poly1305( conteúdo )
 chave       = HKDF-SHA-256( chave da organização, info = "arauto/seal/1" ), 32 bytes
 dado        = "arauto/seal/1" || versão (1 byte) || id da organização (16 bytes)
 autenticado    || identidade pública do aparelho (32 bytes)
 conteúdo    = JSON: v, org, device (a identidade pública), generation,
               id (16 bytes sorteados), issued_at
```

- Continua sendo "lacrado com a chave da organização" (04): cada uso deriva a própria chave dela, e a
  dos recados é outra ([13](13-conversa-entre-hermes.md), seção 6).
- O dado autenticado amarra o lacre à versão, à organização e ao aparelho: um lacre levado a outro
  aparelho não abre.
- O hub guarda só a geração e o **resumo SHA-256 do bloco inteiro** da vigente e da pendente, e a
  maior geração.
- `issued_at` é a hora de emissão. Nenhuma regra do MVP o lê: fica gravado para o ponto em aberto do
  C11.
- O conector guarda o bloco como recebeu, em `credential.seal`, e nunca o interpreta. Só calcula o
  resumo para o `saved`.

## 10. Convite e chave do hub

**Convite:** 32 bytes sorteados, escritos como `arc1` seguido do base32 sem preenchimento, em
minúsculas: 56 caracteres. O hub guarda só o resumo SHA-256 dos 32 bytes. O conector recusa
localmente, com AR-101, um convite fora desse formato.

**Chave pública do hub**, como vai no bloco: `arh1` seguido dos 32 bytes na mesma escrita, também 56
caracteres. Só existe um texto para cada chave. O conector recusa texto fora do formato e chave que
não é de ninguém.

## 11. O soquete local

Entre as cópias do conector na mesma máquina ([11-conector.md](11-conector.md), seção 3).

| Sistema | Como |
|---|---|
| Linux e macOS | Soquete de domínio em `channel.sock`, dentro da pasta do conector. A pasta e o soquete só o dono lê e escreve, e o líder confere que o outro lado é o mesmo usuário. |
| Windows | Canal nomeado, com o nome derivado do caminho da pasta. O líder exige ser a primeira instância e recusa cliente remoto. É plataforma só de desenvolvimento ([decisão 0036](decisoes/0036-windows-so-para-desenvolvimento.md)). |

O líder só cria o soquete depois de pegar a trava, e apaga o que sobrou de um líder morto.

Cada mensagem é um objeto JSON numa linha, com `"v": 1`. Versão ou operação desconhecida recebe
`{"ok":false,"error":"unsupported"}`.

| De | Operação | Resposta |
|---|---|---|
| repassadora | `call`, com `id`, `tool`, `args` | `{"id":1,"ok":true,"content":{}}` ou `{"id":1,"ok":false,"code":"AR-..."}` |
| repassadora | `tools` | A lista de ferramentas liberadas. |
| repassadora | `watch` | A ligação fica aberta, e o líder manda uma linha a cada vez que a lista de ferramentas muda. |
| `status` | `status` | A situação do aparelho e do canal, sem segredo. |
| `join` | `new_request` | O líder passa a acompanhar o pedido novo. |
| `leave` | `leave` | O líder pede o encerramento de cada identidade da pasta e devolve uma resposta por identidade. |
| `join` | `restart` | O líder encerra, para o Hermes iniciá-lo de novo na versão nova. |

Nenhuma operação do soquete devolve a identidade ou a credencial. No modo blindado, `leave` e
`restart` ficam fora do alcance da LLM.

## 12. Limites conhecidos e o que falta provar

Pontos que a revisão levantou e que pedem mudança no [04](04-seguranca-conexao.md). Ficam aqui até
quem mantém o projeto aprovar a redação:

| Ponto | Onde entraria |
|---|---|
| O teste de 3 segundos da conexão antiga pode falhar durante uma transferência grande em rede lenta: a resposta espera atrás do que já está na fila de envio do sistema. A conexão antiga é dada por morta e a nova entra, com o alerta de conexão tomada. | C9, como limite. |
| Um lacre válido, de resumo desconhecido e geração já conhecida, é tratado como reuso (quarentena). Também pode ser sinal de restauração. Guardar o resumo de toda credencial emitida distinguiria os dois. | C7 e C11. |
| As recusas de inscrição são registradas agregadas, uma linha por origem, organização, código e minuto, e não uma por tentativa. No hub isso vale para toda recusa, inclusive o AR-102 repetido; o C3 fala em "registro na auditoria" a cada recusa. A contagem do minuto em que o hub para se perde. | C3. |
| A regra que usaria a data de emissão do lacre continua em aberto. | C11. |
| A primeira reconexão é "na hora". Uma espera sorteada de poucos segundos aliviaria a volta em massa depois de o hub reiniciar; hoje quem alivia é o teto de conexões sem sessão. | C9. |
| O texto ainda fala em "cópia contínua do banco de controle", do desenho com dois arquivos. | C11. |

Pontos que vieram do código do hub (etapas 2 e 2b, 11/11/2026). Cada um diz o que mudaria no 04 e
por quê; até a aprovação, o 04 continua valendo como está:

| Ponto | Onde entraria |
|---|---|
| **Conector antigo demais.** O C8 diz que ele "recebe AR-401 e nada mais". Pela decisão 0030, já decidida, o hub primeiro faz a apresentação sem troca (seção 5.3) e só depois manda AR-401. O hub faz assim. O C8 passaria a descrever essa ordem. | C8, e seção 6, item 3. |
| **`leave` de conector novo demais.** O C8 aceita o `leave` "em qualquer versão". O hub responde AR-402 ao conector novo demais, também no `leave`, porque não existe `hello_ok` válido para uma faixa toda acima da do hub. O C8 passaria a dizer "em qualquer versão abaixo da máxima do hub". | C8. |
| **A recusa AR-710 não relê o estado do aparelho.** O C8 manda conferir o estado "em toda chamada". O hub relê em toda chamada que roda, antes de qualquer outra conferência; a 33ª chamada em andamento é recusada sem tocar o banco. Reler ali faria cada recusa disputar o conjunto de conexões com as 32 chamadas em andamento e quebraria o teto. A varredura fecha em até 2 segundos o aparelho que deixou de servir. Ou o C8 passa a dizer "em toda chamada que roda", ou o hub passa a reler também na recusa: decisão de quem mantém. | C8. |
| **O resumo do convite também é apagado ao cancelar.** O 04 manda apagá-lo quando o convite vence. O hub apaga também quando o convite é cancelado: por convite novo, por revogar ou pelo modo restaurado. Um resumo guardado sem uso só serve a quem lê o banco. | C1 e seção 4. |
| **Em qual gravação o aprovado vira ativo.** O 04 não diz. No hub, quando um aprovado volta apresentando a pendente, a promoção em T1 já é a primeira confirmação, e o aparelho passa a ativo ali; a sessão só começa depois do "gravei" da credencial seguinte. | C5 e C7. |
| **A ordem entre prova e prazo.** O 04 não fixa. O hub olha a prova de restauração antes de aplicar um prazo vencido, para o prazo não esconder a prova. | C7 e C11. |

A provar no MVP, pela suíte: a troca com quedas, atrasos e repetições sorteados; a resposta atrasada
de uma tentativa abandonada; o revogar entre a credencial e o "gravei"; os limites de antes da sessão;
a mensagem cortada em pedaços; e o soquete local nos dois sistemas. A lista única continua em
[12-roteiro.md](12-roteiro.md).

**Andamento, 11/11/2026.** Do lado do hub, a suíte já tem teste para a troca com quedas sorteadas dos
dois lados (com e sem cópia dos arquivos), a resposta atrasada de uma tentativa abandonada, o revogar
entre a credencial e o "gravei", os limites de antes da sessão e a mensagem cortada em pedaços. Os
testes rodam contra um conector de teste, não contra o conector do projeto, que ainda não existe, e
só em Windows. Falta tudo do lado do conector, inclusive o soquete local.

**Limites conhecidos do hub, 11/11/2026:**

- Antes dos cabeçalhos, as conexões TCP não têm teto: só o prazo de 5 segundos as limita.
- O hub dos testes não teve um proxy de verdade na frente; a leitura do endereço encaminhado foi
  testada com um proxy confiável na própria máquina.
- O registro das conexões e a conferência da troca de 24 horas não têm índice por aparelho: cada
  aviso percorre todas as conexões. O custo não foi medido além de 500 conectores.
- A tentativa abandonada é coberta por um teste dirigido e pelas rodadas sorteadas; a janela entre a
  última conferência e o início de T1 não tem teste que a force.

## 13. Pontos em aberto

O código escolheu uma leitura onde o texto diverge dele, ou onde a escolha não é melhora clara. Ficam
aqui as duas leituras, até quem mantém o projeto decidir.

| Ponto | Leitura do texto | Leitura do código |
|---|---|---|
| **Desconhecido com o modo restaurado ligado.** | A tabela da seção 6: um desconhecido com lacre válido é prova de restauração, a não ser que já esteja listado. Com o modo ligado, isso recomeçaria o modo. | Com o modo ligado, o desconhecido é listado e não é prova. Pelo C11, só três coisas recomeçam o modo, e o desconhecido não está entre elas. Evita recomeçar o modo a cada aparelho da linha do tempo perdida. O custo: uma segunda restauração por imagem, feita com o modo ligado, não é percebida por um aparelho que o banco não conhece. Uma saída seria usar a hora de início do modo para separar os dois casos. |
| **Desconhecido de novo depois do expurgo.** | A lista de aparelhos desconhecidos tem guarda de 90 dias ([15](15-mvp.md), seção 2, regra 11). | Depois do expurgo, a mesma identidade volta a ser "listada agora" e, com o modo desligado, vira prova outra vez. É o mesmo ponto em aberto do C11: falta como distinguir a sobra da linha do tempo perdida de uma restauração nova. Hoje o expurgo ainda não existe no hub. |
