# 15 — O primeiro MVP

**Estado: proposta em construção.** Revisada em 10/11/2026, depois da revisão do desenho. O que entra
na primeira versão que funciona de ponta a ponta, com que tecnologia e em que ordem. As decisões
estão em [0031](decisoes/0031-banco-postgresql.md),
[0032](decisoes/0032-conversa-entre-hermes-no-mvp.md), [0033](decisoes/0033-escopo-do-mvp.md),
[0034](decisoes/0034-nomes-em-ingles.md), [0035](decisoes/0035-pilha-do-websocket.md) e
[0036](decisoes/0036-windows-so-para-desenvolvimento.md).

## Resumo

```
 M1 CONEXÃO    convite, código digitado, credencial lacrada, canal cifrado, quarentena,
               revogar, reconectar, sair, líder único, modo restaurado
 M2 CONVERSA   recados entre Hermes, lista de pares, tetos, fila de avisos
 M3 WHATSAPP   motor (falso e WAHA), números, matriz de acessos, ferramentas
 M4 ACORDAR    o conector inicia uma execução no Hermes a partir de um aviso
 M5 ENTREGA    imagem de container, composição com PostgreSQL, teste de carga completo
```

| Pergunta | Resposta |
|---|---|
| O que o MVP faz? | Conecta Hermes ao hub, deixa um Hermes mandar recado a outro e usar números de WhatsApp, e acorda o Hermes quando chega algo. |
| Como se administra? | Por um comando no servidor do hub. O painel, com passkey, vem depois. |
| Onde roda em desenvolvimento? | Direto pelo `cargo`, com um PostgreSQL local. Sem container. |
| Onde roda em produção? | Em container, em Linux, com PostgreSQL, por um arquivo de composição. |
| O que fica de fora? | O painel, o arquivo de mensagens, a assinatura das versões e o modo blindado. |

## 1. O que entra e o que não entra

| Entra | Não entra, e fica para depois |
|---|---|
| Convite, inscrição, código digitado, aprovação | Painel no navegador e passkey ([07-telas.md](07-telas.md)) |
| Canal Noise, troca da credencial, quarentena, revogar, reconectar, sair | Aviso ao admin por fora ([decisão 0025](decisoes/0025-aviso-por-fora-do-painel.md)) |
| Modo restaurado ([04](04-seguranca-conexao.md), C11), com a marca fora do banco | O ponto em aberto do C11 (a data de emissão do lacre) |
| Líder único e repassadoras | Modo blindado |
| Conversa entre Hermes ([13](13-conversa-entre-hermes.md)) | Observador, salas e leitura do texto pelo admin |
| Motor falso e adaptador do WAHA; ler, enviar e modificar | Arquivo de mensagens e busca (a leitura depende do motor no ar) |
| Matriz de acessos, por número e por conversa | Envio de arquivo por endereço |
| Avisos e acordar, com a frase fixa | Lista de versões do programa, com revogação e assinatura |
| Auditoria que só cresce, e alertas lidos por comando | Tetos ajustáveis |
| Teto de envio (20 por minuto, por Hermes e por número) e tetos de recado | Autoridade privada no TLS do conector |

**Administração no MVP.** Não existe endereço de administração na rede. Tudo o que o painel faria é
feito por `arauto-hub admin`, rodado por uma pessoa **na máquina do hub**. Quem roda esse comando já
tem o banco e a chave-mestra nas mãos; ele não cria um caminho mais fraco. O hub e o comando são o
mesmo programa e usam as mesmas funções de gravação.

```
 arauto-hub init                              cria chave-mestra, marca, organização e chave do hub
 arauto-hub serve                             o hub
 arauto-hub migrate --i-have-a-backup         aplica migrações num banco que já tem dados
 arauto-hub admin status                      situação, chave pública do hub, modo restaurado
 arauto-hub admin hermes add|list|remove      as linhas de Hermes
 arauto-hub admin invite NOME                 emite o convite e mostra o bloco (conectar ou reconectar)
 arauto-hub admin approve NOME [CODIGO]       o código é DIGITADO; sem ele, o comando pergunta
 arauto-hub admin deny|revoke NOME
 arauto-hub admin wake NOME on|off            pode ser acordado
 arauto-hub admin peers allow|deny|list       a lista de pares (--wake, --both)
 arauto-hub admin threads [NOME]              fios: quem, quando e quanto; nunca o texto
 arauto-hub admin engine|number|access ...    motor, números e matriz de acessos (M3)
 arauto-hub admin alerts [--ack ID]           os alertas que o painel mostraria
 arauto-hub admin audit [--tail N]
```

- Ao aprovar um reconectar, o comando mostra o último uso do aparelho atual e exige `--recognized`
  ou `--not-recognized` (04, C10). Sem uma das duas, não aprova.
- O código de conferência fica em claro na linha do aparelho até o pedido ser resolvido, e depois é
  apagado. Passado como argumento, fica no histórico do terminal: vale uma vez e morre em 10 minutos.
- O ator gravado na auditoria é "admin local", com o usuário do sistema que rodou o comando, como
  texto declarado.

## 2. Banco: PostgreSQL

O banco passa a ser o PostgreSQL ([decisão 0031](decisoes/0031-banco-postgresql.md)). O modelo é o do
[06-modelo-de-dados.md](06-modelo-de-dados.md), com os nomes em inglês.

| No modelo | No banco | Observação |
|---|---|---|
| organizacao | `organization` | |
| admin | `admin` | No MVP, o "admin local". |
| hermes | `hermes` | O nome segue o formato restrito ([13](13-conversa-entre-hermes.md), seção 2). |
| aparelho | `device` | `slot` (`in_use`, `request` ou nulo) e `first_confirmed_at`. Índice único em (Hermes, `slot`). |
| aparelho_desconhecido | `unknown_device` | |
| convite | `invite` | Só o resumo. |
| conexao_canal | `channel_connection` | Uma linha por conexão conferida de aparelho conhecido. |
| motor, numero, conversa | `engine`, `number`, `chat` | |
| acesso, acesso_conversa | `access`, `access_chat` | |
| evento_motor | `engine_event` | Caixa de entrada. |
| aviso | `notice` | Caixa de saída, com `kind` (`whatsapp` ou `peer`), o alvo de cada tipo, `not_before` e `leased_until`. |
| auditoria | `audit` | Gatilhos proíbem alterar, apagar e truncar. |
| chamada_ferramenta | `tool_call` | Retenção curta. |
| instalacao, chave_hub | `installation`, `hub_key` | |
| (novo) alerta | `alert` | O que o painel mostraria. |
| (novo) ligação | `peer_link` | [13](13-conversa-entre-hermes.md) |
| (novo) fio, recado | `peer_thread`, `peer_message` | O texto do recado vai cifrado. |
| (novo) retenção | `retention` | Prazo de guarda por tabela. |

O `slot` do aparelho: `request` enquanto ele não confirmou a primeira credencial e o estado não é
final; `in_use` depois da primeira confirmação, em ativo, suspenso ou quarentena; nulo em estado
final. No reconectar, o antigo sai do `slot` antes de o novo entrar, na mesma gravação.

Regras que valem para o banco:

1. **Um banco só.** Os dois arquivos do desenho anterior existiam porque o SQLite tem um escritor.
2. **A organização vem resolvida.** Só uma lista fechada de funções acha a organização a partir de
   fora: pela identidade do aparelho, pelo resumo do convite e pelo segredo de eventos do motor. As
   tarefas do sistema que atravessam organizações (filas, prazos, expurgo, marca) têm nome e também
   estão na lista. Toda outra função do repositório exige a organização já resolvida. Um teste lê o
   código do repositório e falha se aparecer uma função fora da lista sem esse parâmetro.
3. **Identificadores sorteados no hub** (UUID versão 4). A única sequência é a da auditoria.
4. **O tempo é do hub.** Colunas de tempo são inteiros, em milissegundos UTC. Nenhuma coluna tem
   valor padrão de tempo e nenhum SQL usa `now()`: um teste varre as migrações. Toda conexão define
   `lock_timeout` (5 s) e `idle_in_transaction_session_timeout` (30 s).
5. **As filas ficam no banco.** Pegar um item é um `UPDATE` curto, sobre uma seleção com
   `FOR UPDATE SKIP LOCKED`, que marca o item como enviado com um prazo de posse, e é confirmado
   antes do envio. Posse vencida volta a pendente. Nenhuma trava espera a confirmação do conector. O
   trabalhador só busca avisos de Hermes com sessão aberta.
6. **Gravação que muda aparelho relê antes.** Aprovar, julgar, promover, substituir e revogar abrem
   transação, travam a linha do Hermes e depois as dos aparelhos dele, releem e só então gravam
   ([14](14-protocolo-do-canal.md), seção 6). Nenhuma transação fica aberta durante espera de rede.
7. **Durabilidade.** As transações de evento de segurança e as da troca rodam com
   `SET LOCAL synchronous_commit = on`. Na partida, o hub confere `fsync` e `full_page_writes` e
   recusa subir com eles desligados, salvo com uma opção explícita de teste. Um commit perdido da
   troca pareceria restauração e suspenderia todos. Os metadados de `tool_call` podem ir sem espera.
8. **Migrações embutidas no programa**, só para a frente. `init` aplica tudo num banco vazio. Num
   banco com dados, `serve` não sobe com migração pendente: quem instala tira a cópia de segurança e
   roda `migrate --i-have-a-backup`. O `admin` recusa rodar contra um banco de outra versão.
9. **A marca fica fora do banco**, ao lado da chave-mestra (04, C11). Uma função única, usada pelo hub
   e pelo comando de administração, cuida dela: sob uma trava de arquivo entre processos, confere que
   o banco não está atrás da marca antes de gravar um evento de segurança, e depois avança a marca.
   A marca só aumenta. Gravações que esperam se juntam numa só. O hub repete a conferência na partida
   e a cada varredura: um banco restaurado com o hub no ar é percebido em segundos, e um `admin`
   rodado antes do `serve` não apaga a prova.
10. **A auditoria só cresce**, e guarda só o que muda quem pode o quê. Os gatilhos protegem contra
    erro do programa; quem tem o banco consegue apagar. Recusas repetidas e recados não geram um
    evento cada ([13](13-conversa-entre-hermes.md), seção 7).
11. **Expurgo.** Uma tarefa do hub apaga em lotes pequenos, com a hora do hub, pelo prazo de cada
    tabela: recados, 30 dias; chamadas de ferramenta, 7; avisos resolvidos, 7; eventos do motor já
    tratados, 1; conexões e aparelhos desconhecidos, 90.
12. **O banco é acessado direto**, sem agrupador de conexões em modo de transação. O conjunto de
    conexões tem tamanho fixo (16 de partida), e esperar por uma tem prazo de 5 segundos.

**Como o hub fica sabendo.** O que o próprio hub enfileira acorda o trabalhador por um sinal em
memória. O `NOTIFY` do PostgreSQL serve só entre processos: o comando de administração grava, e
avisa o hub com um texto curto, do tipo `revoked:{id}`. O hub escuta numa conexão própria, fora do
conjunto. Como esse aviso não é guardado, o hub também **varre a cada 2 segundos** e toda vez que a
escuta reconecta: relê o estado dos aparelhos com conexão aberta, a lista de ferramentas de cada
sessão e as filas. É a varredura, não o aviso, que garante o revogar em menos de 5 segundos.

Um segundo hub servindo a mesma organização não é tratado no MVP: o registro de conexões vive na
memória de um processo.

## 3. Websocket: como se busca desempenho

A escolha está na [decisão 0035](decisoes/0035-pilha-do-websocket.md). O que pesa no hub não é o
websocket: é a cifra de cada quadro e o banco.

| Cuidado | Como |
|---|---|
| Servidor | `axum` sobre `hyper`, no `tokio` com vários núcleos. O laço que aceita conexões é do projeto: tem prazo para os cabeçalhos, teto de conexões e um sinal de cancelamento por conexão. |
| Uma conexão, duas tarefas | Uma lê, outra escreve. Só a que escreve cifra ([14](14-protocolo-do-canal.md), seção 3). |
| Duas filas de saída | Controle (16 quadros) e dados (64 quadros). A tarefa que escreve olha o controle antes de cada quadro. |
| Contrapressão | Quem produz dado espera lugar na fila. A conexão só é fechada quando a escrita fica 30 segundos sem andar, ou quando a fila de controle enche. Rede lenta não expulsa ninguém. |
| Cifra sem trava | O Noise é usado no modo em que o contador é passado à mão, um por sentido. A recusa de quadro repetido ou fora de ordem é código do projeto, com teste. |
| Memória | Limites por fase e um orçamento do hub inteiro para mensagens em remontagem ([14](14-protocolo-do-canal.md), seção 4). Mensagem acima de 256 KiB é interpretada fora das tarefas de rede. |
| Sem compressão | O conteúdo já vai cifrado. |
| Sem espera no envio | `TCP_NODELAY` ligado em cada conexão aceita. |
| Tabela de conexões | Um mapa repartido, por aparelho. |
| Banco | Conjunto de tamanho fixo; consultas curtas, por índice. |
| Autorização a cada chamada | Uma consulta por índice, sem memória intermediária. É a regra 9 do projeto; mede-se antes de otimizar. |
| Volta em massa | O teto de conexões sem sessão segura a primeira onda; quem é recusado volta com espera sorteada. As gravações da marca se juntam. |
| Parada em ordem | O hub para de aceitar, manda `closing` a cada conexão, espera até 8 segundos e sai. O sinal é SIGTERM, ou Ctrl+C no desenvolvimento. |

**Meta e teste de carga.** "Ferramenta local" é a que o hub responde sem falar com o motor.

| Item | Cenário e critério |
|---|---|
| Carga | 2.000 conectores simulados, em outro processo, cada um com a própria identidade. |
| Regime | Batimento; uma chamada de leitura local a cada 10 segundos por conector; um em cada dez manda um recado por minuto. |
| Volta em massa | O hub reinicia e todos reconectam. |
| Aprovado quando | Leitura local: 95% em até 20 ms e 99% em até 100 ms. Recado: 95% em até 50 ms. As 2.000 sessões de volta em até 60 segundos, sem nenhuma quarentena. Até 100 KiB de memória por sessão parada. |

São metas: só viram dado quando o teste rodar, e o resultado vale para a máquina em que rodou. O M1
já roda uma versão menor (500 conectores: volta em massa, batimento e leitura local).

## 4. O código

Um espaço de trabalho em Rust, com o `Cargo.lock` versionado.

```
crates/
  arauto-protocol/    o que os dois programas compartilham: quadros, mensagens, Noise,
                      convite, código de conferência, formato de nome, limites e prazos,
                      catálogo de erros e catálogo de ferramentas
  arauto-platform/    o que muda de um sistema para outro: gravação durável, trava de
                      arquivo, soquete local, sinais de parada
  arauto-hub/         o hub: servidor e comando de administração
  arauto-connector/   o conector
  arauto-testkit/     só para testes: motor falso, Hermes falso, gerador de carga
deploy/               Dockerfile e arquivo de composição
```

Módulos do hub ([03-arquitetura.md](03-arquitetura.md), seção 3): `enrollment`, `channel`, `vault`,
`core`, `engine`, e o novo `peers`. O `panel` e o `archive` não existem no MVP.

| Peça | Biblioteca |
|---|---|
| Servidor e websocket do hub | `axum`, `hyper`, `hyper-util`, `tokio` |
| Websocket do conector | `tokio-tungstenite`, com `rustls` e as autoridades públicas embutidas |
| Banco | `sqlx` (PostgreSQL), consultas em tempo de execução |
| Noise | `snow` |
| Lacre e cifra em repouso | `chacha20poly1305` |
| Resumo e derivação | `sha2`, `hkdf` |
| Sorteio | `getrandom` (gerador do sistema) |
| Segredo em memória | `zeroize`; comparação com `subtle` |
| Mensagens | `serde`, `serde_json`; base32 e base64 com `data-encoding` |
| Servidor MCP por stdio | Escrito no projeto: é pequeno e é o texto que a LLM lê |
| Soquete local e trava | `tokio` e a biblioteca padrão, dentro de `arauto-platform` |
| Cliente do motor e da API local | `reqwest` |
| Linha de comando e registro | `clap`, `tracing` |

A versão mínima do Rust é a fixada no repositório. Em produção o conector é um binário estático para
Linux x86_64.

**Configuração**, por variáveis de ambiente:

| Programa | Variável | Para quê |
|---|---|---|
| hub | `ARAUTO_DATABASE_URL` | O banco. |
| hub | `ARAUTO_DATA_DIR` | Chave-mestra e marca. Fora do volume do banco. |
| hub | `ARAUTO_LISTEN`, `ARAUTO_LISTEN_INTERNAL` | A porta dos conectores; a porta interna (saúde e eventos do motor). |
| hub | `ARAUTO_PUBLIC_URL` | O endereço que vai no bloco. |
| hub | `ARAUTO_CLIENT_IP_SOURCE`, `ARAUTO_TRUSTED_PROXIES` | De onde vem o endereço do conector: `socket`, ou `forwarded` com a lista de proxies confiáveis. Obrigatória: sem ela o `serve` não sobe. |
| conector | `ARAUTO_HOME` | A pasta do conector. Padrão: `~/.arauto`. |
| conector | `ARAUTO_HERMES_BIN` | O comando do Hermes usado no registro. Padrão: `hermes`. |

Quando a pasta não é a padrão, o registro no Hermes leva `--home` nos argumentos: o Hermes inicia o
conector com um ambiente filtrado, e a variável não chegaria.

## 5. Como rodar em desenvolvimento

Sem container. O que é preciso: o Rust da versão fixada e um PostgreSQL 16 local.

```
 1 subir o PostgreSQL local e criar o banco de desenvolvimento
 2 cargo run -p arauto-hub -- init            não refaz o que já existe
 3 cargo run -p arauto-hub -- serve           o hub
 4 cargo run -p arauto-hub -- admin ...       cria Hermes, emite convite, aprova, liga os pares
 5 cargo run -p arauto-connector -- join ...  uma pasta por Hermes de teste, com ARAUTO_HOME
 6 cargo test --workspace --locked            a suíte
```

- **Dois conectores na mesma máquina:** cada um com a sua `ARAUTO_HOME`. A pasta define o soquete
  local e a trava do líder.
- **Sem Hermes de verdade:** o `arauto-testkit` traz um Hermes falso, que aceita o registro, inicia
  o conector por stdio em quantas cópias se pedir e tem a API local que acorda. `ARAUTO_HERMES_BIN`
  aponta para ele. O `join --no-register` pula o registro.
- **Zerar o ambiente:** `arauto-hub dev-reset` apaga o banco de desenvolvimento **e** a marca,
  juntos. Apagar só um dos dois ligaria o modo restaurado, que é o comportamento certo.
- **Windows** é plataforma de desenvolvimento do hub e do conector
  ([decisão 0036](decisoes/0036-windows-so-para-desenvolvimento.md)). O que muda:

| Peça | No Linux | No Windows de desenvolvimento |
|---|---|---|
| Gravação durável | Temporário, gravação forçada do arquivo, troca de nome, gravação forçada da pasta | O mesmo, sem a gravação da pasta |
| Trocar o programa em uso | Troca de nome por cima | Renomeia antes o que está em uso |
| Soquete local | Soquete de domínio, só do dono, com conferência do usuário | Canal nomeado, primeira instância, sem cliente remoto |
| Trava do líder | Trava de arquivo | Trava de arquivo, num arquivo vazio só para isso |
| Pasta só do dono | Modo 0700 | A permissão herdada do perfil do usuário |
| Parada | SIGTERM | Ctrl+C |

**A suíte.**

- Os testes com banco usam o endereço em `ARAUTO_TEST_DATABASE_URL`. Sem ela **falham**, com uma
  mensagem que diz o que falta: não são pulados em silêncio.
- As migrações rodam uma vez, num banco-modelo. Cada teste que precisa de banco próprio cria o seu a
  partir do modelo; os demais dividem um banco e se isolam por organização. O usuário do banco de
  teste precisa poder criar banco. Bancos de teste antigos são apagados na partida.
- Cada teste tem a sua pasta temporária de chave-mestra e marca, e um conjunto de 2 ou 3 conexões.
- **A regra da troca é uma função pura**: o estado do aparelho mais um evento dão a decisão e as
  gravações. É ela que passa por dezenas de milhares de rodadas sorteadas, em memória, com relógio e
  rede simulados. A semente sai na falha. Algumas centenas de rodadas contra o PostgreSQL de verdade
  conferem que o SQL segue o mesmo modelo.
- Todo prazo vem de configuração e de um relógio que o teste controla.

## 6. Como entregar em produção

Em container ([decisão 0006](decisoes/0006-instalado-no-servidor-de-cada-um.md)).

| Serviço | O que é |
|---|---|
| `hub` | A imagem do projeto. Publica só a porta dos conectores. |
| `postgres` | PostgreSQL 16, com volume próprio, que precisa honrar a gravação forçada. |
| proxy | De quem instala. Termina o TLS da porta dos conectores. |
| motor | De quem instala, referenciado por endereço. **Não vai na imagem nem na composição.** |

- A chave-mestra e a marca ficam num volume **separado** do volume do banco e fora da cópia de
  segurança dele.
- **Cópia de segurança:** a do PostgreSQL, contínua (arquivamento do registro de transações) ou, no
  mínimo, diária. Restaurar liga o modo restaurado, e todos os Hermes reconectam (C11).
- **O proxy precisa:** repassar a troca para websocket, ter prazo de inatividade acima de 90
  segundos e não segurar dados em memória. O hub é configurado com a lista de proxies confiáveis
  para ler o endereço de cada conector.
- **Os eventos do motor** chegam pela porta interna, que não é publicada: o motor alcança o hub por
  uma rede compartilhada da composição, e cada evento traz o segredo de eventos.
- A composição declara o limite de arquivos abertos do hub e o prazo de parada.
- A imagem traz o conector para Linux x86_64, servido no endereço de download, com o resumo SHA-256
  calculado na construção.

## 7. Marcos e como cada um se verifica

O critério de pronto de cada fase do [roteiro](12-roteiro.md) continua valendo. O MVP os reagrupa.

| Marco | Entra | Pronto quando |
|---|---|---|
| **M1 Conexão** | Quadros, fluxos e limites; aperto de mãos; inscrição e código; aprovar por comando; a troca; sessão, batimento e troca de 24 horas; quarentena, revogar, reconectar e sair; os prazos (convite, pendente, aprovado, inatividade); auditoria, marca e modo restaurado; alertas; líder, repassadoras e soquete local; servidor MCP com `arauto_status`; o comando `status`; Hermes falso; carga reduzida. | Um conector entra com convite e código. A troca passa pelas rodadas sorteadas sem trancar de fora um aparelho legítimo, sem aceitar credencial reusada e sem deixar uma cópia passar despercebida quando o verdadeiro volta. Têm teste: a resposta atrasada de uma tentativa abandonada; revogar entre a credencial e o "gravei"; os limites de antes da sessão; revogar em menos de 5 segundos com o aviso perdido; banco restaurado com o hub no ar; `admin` antes do `serve` depois de restaurar. |
| **M2 Conversa** | Lista de pares; fios e recados; os tetos; as quatro ferramentas; a fila de avisos, com posse, agrupamento e reavaliação; `notice` e `notice_ack` contra um destino de teste; a lista de ferramentas que muda com a lista de pares. | Têm teste: sem ligação não manda, e a resposta é a mesma de nome inexistente; não lê nem lista fio de outros; não responde sem a ligação de volta; tirar a ligação corta o não lido; o laço sem `thread` para no teto; cada teto; o aviso não carrega texto; o conector recusa `from` fora do formato. |
| **M3 WhatsApp** | A interface do motor, o motor falso e o adaptador do WAHA; números; matriz de acessos; as ferramentas de WhatsApp; teto de envio; caixa de entrada de eventos; avisos de mensagem. | Têm teste: um Hermes não alcança o número de outro; `to` e `message_id` maliciosos são recusados; tirar um acesso vale na chamada seguinte. Tudo contra o motor falso. |
| **M4 Acordar** | A chamada à API local do Hermes; a memória de avisos tratados; o teto de acordadas do conector. | Com quedas sorteadas do hub e do conector, nenhum aviso some antes de expirar e nenhum aviso repetido inicia uma segunda execução. |
| **M5 Entrega** | Dockerfile, composição, endereço de download, guia de instalação e o teste de carga completo. | A imagem sobe com a composição, o hub passa na conferência de saúde e o teste de carga atinge os critérios da seção 3, ou o resultado fica registrado com o que faltou. |

Valem as regras de verificação do roteiro: motor falso e Hermes falso, rodadas sorteadas, mutantes
nas regras de segurança e o canário de segredo.

## 8. O que o MVP não prova

- **As provas "no Hermes".** O registro pelo comando oficial, quantas cópias do conector ele inicia,
  se ele atualiza a lista de ferramentas quando ela muda, e a chave e o formato da API local
  dependem de um Hermes de verdade. Até lá o conector segue o que está em
  [hermes-fatos](../referencia/hermes-fatos.md), e "ser acordado" só é dado como funcionando contra
  o Hermes falso.
- **O caminho de Linux.** Enquanto o desenvolvimento for só em Windows, o código específico de Linux
  é conferido pelo compilador e nunca executado. Ele precisa rodar em Linux, com a suíte inteira,
  antes de qualquer uso de verdade.
- **O container.** A imagem e a composição são escritas no M5; só valem depois de construídas e
  testadas num servidor.
- **O motor de verdade.** O adaptador do WAHA é testado contra o comportamento registrado em
  [waha-fatos](../referencia/waha-fatos.md). Não há teste automático em número real.
