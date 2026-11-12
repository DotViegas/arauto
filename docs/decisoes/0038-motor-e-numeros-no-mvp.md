# 0038 — O motor e os números no primeiro MVP

**Estado:** proposta · **Data:** 11/11/2026, com o código do motor e dos números no hub (M3, primeira
parte)

## Contexto

O [15-mvp.md](../15-mvp.md) pede, no M3, a interface do motor, o adaptador do WAHA, o motor falso,
a caixa de entrada de eventos e os números. O [06-modelo-de-dados.md](../06-modelo-de-dados.md),
seções 4 e 6, o [10-maquinas-de-estado.md](../10-maquinas-de-estado.md), seção 4, e as armadilhas
de [waha-fatos](../../referencia/waha-fatos.md) dizem o que cada peça faz, mas deixam escolhas de
código em aberto. Esta decisão registra as escolhas e as leituras feitas onde o texto é omisso ou
diverge. Os formatos da API do WAHA estão em waha-fatos, seção "Os formatos que o hub usa", lidos
na documentação pública do autor e ainda não conferidos contra um motor de verdade.

## Decisão

- **O cliente do motor é o HTTP/1.1 do `hyper`**, uma conexão por chamada, com prazo para abrir e
  prazo para a chamada inteira, teto de tamanho conferido no tamanho declarado e de novo na leitura,
  e o tipo de conteúdo conferido. O 15, seção 4, dizia `reqwest`; ele traria pacotes novos ao
  `Cargo.lock`. Não há TLS até o motor: ele só fica na rede interna
  ([0022](0022-motor-sem-porta-publica.md)).
- **Um motor por organização** (`engine.organization_id` único). O endereço e o endereço dos
  eventos ficam em claro; a chave da API e o segredo dos eventos ficam lacrados pela caixa preta,
  com a chave-mestra, nas colunas `api_key_enc` e `events_secret_enc` (o 06 diz `_cifrado`; o código
  já usava `_enc` desde a 0001). Mudar qualquer um aumenta `config_version`, e o hub refaz o cliente
  e reaplica a configuração de cada sessão do motor.
- **O segredo dos eventos é sorteado pelo hub** (32 bytes) e nunca mostrado: o hub o põe ele mesmo
  na configuração de cada sessão do motor, como o texto hexadecimal. A assinatura é HMAC-SHA-512 do
  corpo cru, em hexadecimal, no `X-Webhook-Hmac`, conferida em tempo constante. Trocar o segredo é
  `admin engine new-events-secret`; a supervisão leva o novo a todas as sessões.
- **A caixa de entrada** é `POST /v1/engine/{motor}/events`, só na porta interna. O motor nomeado
  no endereço é a entrada da organização "pelo segredo de eventos do motor" (15, seção 2, regra 2):
  nada da organização é usado antes de a assinatura conferir, e motor desconhecido recebe a mesma
  resposta que assinatura errada. Ordem: tamanho declarado, tipo de conteúdo, corpo lido com teto e
  prazo, assinatura, formato estrito, janela de 12 horas, gravação. O evento é gravado antes da
  resposta.
- **Um evento é uma linha.** Uma mensagem é identificada pelo identificador dela, não pelo do
  evento: a mesma mensagem vinda pela caixa e pela busca da partida é uma linha só. Os outros tipos,
  pelo identificador do evento. A chave fica como resumo com chave da organização, e o corpo,
  cifrado com a chave da organização. A janela de 12 horas fica abaixo do prazo de guarda do evento
  tratado (1 dia): um evento gravado por alguém não volta a valer depois de expurgado.
- **Telefones e identificadores de conversa são dado pessoal de terceiros.** O 06 lista `telefone`
  como coluna comum; o hub o guarda cifrado, com um resumo com chave para achar o número, e faz o
  mesmo com os identificadores de conversa (`chat_engine_id`) e com o nome mostrado da conversa.
- **Os estados do número** são os do 10, seção 4. A leitura do código: o estado da sessão do motor
  mais antigo que chega depois de um mais novo é ignorado (`status_at`); um pareamento tem prazo
  (`pairing_until`, 3 minutos); "pareando" com uma conta que não é o telefone esperado, ou que o
  motor não nomeia, vai a "número trocado".
- **Remover e desfazer o pareamento sem arquivo de mensagens.** As armadilhas mandam importar o
  histórico antes de sair do WhatsApp, e o arquivo de mensagens não está no MVP. Por isso o comando
  exige uma de duas palavras: `--keep-in-engine` (a sessão é parada e fica no motor, com o
  histórico; o número sai do hub) ou `--erase-engine-history` (sai do WhatsApp e o motor apaga o
  histórico). Desfazer o pareamento de um número trocado exige a segunda.
- **A auditoria de segurança** ganha, além da lista do 15, seção 2, regra 10: configurar o motor,
  trocar a chave, trocar o segredo dos eventos, adicionar, remover e confirmar um número. São o que
  muda quem pode o quê no WhatsApp. Pedir pareamento, a mudança de estado vinda do motor e a correção
  de uma sessão pela supervisão não são.
- **A supervisão**, na partida e a cada 5 minutos: confere a versão (alerta uma vez quando muda para
  uma que não está na lista testada, hoje só `2026.10.1` com GOWS), cria ou corrige a sessão de cada
  número (alerta quando corrige algo posto por fora), religa a sessão parada de número conectado,
  encerra pareamento vencido e, para cada número conectado, busca no motor as mensagens das últimas
  24 horas (no máximo 50 conversas e 100 mensagens por conversa) que a caixa não recebeu.
- **O limite do WhatsApp** (um 429 do motor) é lido como AR-704. O 08 reserva o AR-704 para o teto
  de envio do hub; as duas situações pedem ao Hermes a mesma coisa (esperar).
- **O QR do pareamento** é desenhado no terminal da máquina do hub por um codificador escrito no
  projeto (`admin/qr.rs`), sem dependência nova. O QR e o código aparecem uma vez e não são
  guardados nem registrados.

## Consequências

- Os avisos de mensagem não nascem aqui: as linhas de `engine_event` de mensagem, reação, edição e
  apagamento ficam com `processed_at` vazio para a fila de avisos. As de estado da sessão são
  tratadas na mesma transação.
- Sem o arquivo de mensagens, um número removido com `--keep-in-engine` deixa a sessão no motor,
  com o histórico, até alguém decidir. O hub não a reaproveita: o nome da sessão nunca volta.
- O formato da API do WAHA é o da documentação pública. Até o teste de contrato rodar contra um
  motor de verdade, cada "a conferir" de waha-fatos é uma suposição, escrita na leitura mais segura.
- Alternativas consideradas: o `reqwest`, pelo motivo acima; a assinatura conferida por um
  cabeçalho fixo em vez do HMAC, que o motor documenta e que não prova o corpo; e o segredo dos
  eventos digitado pelo admin, que faria um segredo duradouro passar por uma pessoa.
