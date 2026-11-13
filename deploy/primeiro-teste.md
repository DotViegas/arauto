# Roteiro do primeiro teste num servidor

Para quem instala o Arauto pela primeira vez com um Hermes de verdade e um motor de WhatsApp de
verdade. Cada passo diz o que fazer e o que tem de acontecer. Onde o resultado for outro, **pare e
anote**: o que saiu na tela, o registro do hub e a hora.

O guia de instalação é o [docker/README.md](docker/README.md); este roteiro segue a mesma ordem e
acrescenta as conferências.

```
 FASE 0  antes de começar          FASE 5  o número de WhatsApp
 FASE 1  subir o hub               FASE 6  acordar o Hermes
 FASE 2  o proxy                   FASE 7  recados entre dois Hermes
 FASE 3  o primeiro Hermes         FASE 8  o que não pode acontecer
 FASE 4  o motor de WhatsApp       FASE 9  quedas, cópia e fechamento
```

Nos comandos, `dc` é `docker compose --env-file arauto.env`, e `adm` é
`dc exec hub arauto-hub admin`.

## O que este teste precisa responder

Cinco coisas nunca rodaram fora dos testes automáticos. Elas são a razão do teste, e cada uma tem
um passo marcado com **[PROVA]**.

| # | O que nunca foi provado | Passo |
|---|---|---|
| 1 | A imagem constrói e a composição sobe. | 1.2 a 1.5 |
| 2 | O conector para Linux roda na máquina do Hermes e fala `wss://` (TLS). | 3.2 e 3.3 |
| 3 | O `join` registra o conector no Hermes, e o Hermes o inicia. | 3.4 a 3.6 |
| 4 | O hub fala com o WAHA de verdade: parear, receber e enviar. | 4 e 5 |
| 5 | O conector acorda o Hermes pela API local. | 6 |

## Fase 0: antes de começar

- [ ] **0.1** Servidor Linux x86_64 com Docker e Compose; um nome de domínio apontando para ele; um
  proxy com TLS e certificado de autoridade pública (autoassinado não serve: o conector recusa).
- [ ] **0.2** Na máquina do Hermes, como o usuário do Hermes: `uname -m` responde `x86_64`;
  `ldd --version` mostra glibc 2.36 ou mais nova; `which hermes` responde um caminho. Anote os três.
  Se `which hermes` não responder, você vai precisar de `ARAUTO_HERMES_BIN` no passo 3.3.
- [ ] **0.3** Os relógios do servidor, da máquina do Hermes e do motor estão certos (NTP). O hub
  recusa evento do motor com hora fora da janela.
- [ ] **0.4** Um número de WhatsApp de teste, que possa ser desconectado e reconectado, e um segundo
  celular para mandar mensagens a ele. Nunca um número de uso real.

## Fase 1: subir o hub

- [ ] **1.1** `cp arauto.env.example arauto.env` e preencher. Senha do banco só em hexadecimal
  (`openssl rand -hex 32`): ela vai dentro de um endereço, e `@`, `/`, `#` ou `%` quebram a conexão.
  `git status` não mostra o `arauto.env`.
- [ ] **1.2 [PROVA]** `dc build` termina sem erro. Anote o tempo. É a primeira vez que a imagem é
  construída, e a primeira vez que o conector é compilado com TLS.
- [ ] **1.3** De propósito, `dc up -d` **antes** do `init`. Esperado: o hub fica reiniciando, e
  `dc logs hub` diz que o banco não está instalado e manda rodar o `init`. Nada é criado.
- [ ] **1.4** `dc run --rm hub init`. Esperado: "The hub is installed." e a chave pública do hub.
  Rodar de novo: "already installed", com a mesma chave.
- [ ] **1.5 [PROVA]** `dc up -d` e `dc ps`. Esperado: `hub` como `healthy` em até um minuto.
  `dc logs hub` mostra "the hub is up" com os dois endereços.
- [ ] **1.6** `adm status`. Esperado: banco atual, marca consistente, modo restaurado desligado.
- [ ] **1.7** `dc exec hub ls /usr/share/arauto/download`. Esperado: só `arauto-connector-linux-x86_64`.

## Fase 2: o proxy

- [ ] **2.1** Configurar o proxy ([proxy/README.md](proxy/README.md)): só `/v1/channel` e
  `/v1/download/` chegam ao hub; websocket; prazo de 120 segundos; o cabeçalho `X-Forwarded-For`
  **substituído** pelo endereço de quem chegou.
- [ ] **2.2** De fora do servidor: `curl -I https://SEU-ENDERECO/` responde 404; as portas 8443,
  8444 e 8445 não respondem direto; `curl -I https://SEU-ENDERECO/sign-in` responde 404.
- [ ] **2.3** `curl -fsSLO https://SEU-ENDERECO/v1/download/arauto-connector-linux-x86_64` baixa o
  arquivo. Guarde o `sha256sum` dele para o passo 3.1.
- [ ] **2.4** `dc logs hub` **não** tem a linha "a connection that is not from a trusted proxy
  brought X-Forwarded-For". Se tiver, o endereço que ela mostra é o que falta em
  `ARAUTO_TRUSTED_PROXIES`: acerte, `dc up -d` e repita o 2.3.

## Fase 3: o primeiro Hermes

- [ ] **3.1** `adm hermes add atendimento` e `adm invite atendimento`. Esperado: o bloco com o
  endereço do hub (`wss://...`), a chave do hub, o convite, **uma** linha `connector:`, o `sha256:`
  igual ao do passo 2.3, e um comando só, encadeado. O convite é segredo: não o cole em relatório.
- [ ] **3.2 [PROVA]** Na máquina do Hermes, como o usuário do Hermes, antes de colar o bloco:
  baixe o arquivo à mão e rode `./arauto-connector-linux-x86_64 --version`. Esperado: a versão.
  Erro de GLIBC ou "not found" quer dizer que o programa não serve para essa máquina: pare.
- [ ] **3.3 [PROVA]** Cole o comando encadeado do bloco (pelo canal de comando do Hermes, ou no
  terminal do usuário dele). Esperado: o arquivo baixa, o resumo confere, e o `join` mostra
  "VERIFICATION CODE: NNN-NNN". É a primeira conexão `wss://` de verdade. Se sair `AR-601`, veja a
  tabela do guia; o pedido continua aberto.
- [ ] **3.4 [PROVA]** `hermes mcp list` mostra a entrada `arauto`, com o programa
  `~/.arauto/arauto-connector` e o argumento `mcp`, sem nenhum segredo. Anote o texto exato da
  entrada.
- [ ] **3.5** No servidor: `adm approve atendimento NNN-NNN`. Depois `adm hermes list`. Esperado:
  aprovado e, em até um minuto, ativo.
- [ ] **3.6 [PROVA]** Na máquina do Hermes: `~/.arauto/arauto-connector status`. Esperado: canal em
  sessão, nenhum segredo na tela. No Hermes, a ferramenta `arauto_status` aparece e responde.
- [ ] **3.7** `ls -la ~/.arauto ~/.arauto/devices/*`. Esperado: pastas só do dono (700) e os
  arquivos `identity.key`, `credential.seal` e `hub.json` só do dono (600).
- [ ] **3.8** `adm audit --tail 20`. Esperado: a conexão do Hermes aparece com o endereço **de
  fora** da máquina dele, nunca `172.x` nem o do proxy.

## Fase 4: o motor de WhatsApp

- [ ] **4.1** Subir o WAHA na rede `arauto-engine`, **sem porta pública**. Anote a versão.
- [ ] **4.2** `docker run --rm --network arauto-engine curlimages/curl -s http://hub:8444/health`
  responde `ok`. Sem isso as mensagens chegam com minutos de atraso e nada acusa.
- [ ] **4.3** `adm engine set --url http://waha:3000 --events-url http://hub:8444`, depois
  `dc exec -it hub arauto-hub admin engine set-key` e colar a chave da API do motor.
- [ ] **4.4 [PROVA]** `adm engine health` e `adm engine version`. Esperado: saudável, e a versão.
  Se a versão não for uma das conferidas, o hub avisa e cria um alerta: seguir é um risco sabido.
- [ ] **4.5** Mandar um evento sem assinatura ao hub, de dentro da rede do motor. Esperado: recusa,
  e `dc logs hub` mostra "an event of the engine was refused" com o motivo.

## Fase 5: o número de WhatsApp

- [ ] **5.1** `adm number add balcao +55DDDNUMERO`. Esperado: adicionado, sem parear, com a sessão
  já criada no motor.
- [ ] **5.2 [PROVA]** Antes de parear, guarde a resposta crua de `GET /api/sessions/NOME` do WAHA.
  Depois, `dc exec -it hub arauto-hub admin number pair balcao` num terminal de fundo escuro com
  80 colunas por 40 linhas. Esperado: o QR aparece; ao escanear, "is connected". Se o QR não ler,
  repita com `--code`.
- [ ] **5.3** Logo depois, `adm number list`. Esperado: `connected`. Se a tela disser "another
  telephone" com o telefone certo, **não rode `number confirm`**: rode `adm number unpair balcao --erase-engine-history`, guarde a
  resposta crua da sessão (o campo `me`) e pare este número. O hub espera `me.id` como
  `digitos@c.us`.
- [ ] **5.4 [PROVA]** `adm wake atendimento on` e
  `adm access grant atendimento balcao --scope all --read --send --wake`. Pelo Hermes:
  `whatsapp_list_chats` e `whatsapp_read_chat`. Esperado: as conversas e as mensagens do número.
- [ ] **5.5 [PROVA]** Pelo Hermes, `whatsapp_send_text` para o segundo celular. Esperado: a
  mensagem chega **uma vez**, e a resposta traz `message_id`. Anote quanto demorou.
- [ ] **5.6 [PROVA]** Do segundo celular, mande uma mensagem ao número. No banco, conte as
  conversas dessa pessoa (`SELECT id, hidden_id FROM chat`). Esperado: **uma** conversa. Se
  aparecerem duas, ou uma com `hidden_id` verdadeiro, o motor nomeia a mesma pessoa de dois
  jeitos: anote. Nesse caso um acesso que tenha linhas de conversa não lê nem é acordado por essa
  conversa até ela ganhar uma linha própria, e o registro do hub não acusa erro.

## Fase 6: acordar o Hermes

- [ ] **6.1** Sem mexer em nada: `~/.arauto/arauto-connector status`. Esperado: "Waking allowed by
  the admin: yes" e "Key of the local API of Hermes: not configured". Mande uma mensagem do segundo
  celular. Esperado: o Hermes **não** acorda. É a linha de base.
- [ ] **6.2 [PROVA]** Ponha na entrada `arauto` do Hermes as variáveis `ARAUTO_HERMES_API_KEY` e
  `ARAUTO_HERMES_API_URL` (com o número da máquina, como `http://127.0.0.1:8642/v1/runs`) e reinicie
  o servidor de ferramentas. `status` passa a dizer "configured". Anote **como** a chave ficou na
  entrada: em claro ou por referência a uma variável do Hermes.
- [ ] **6.3 [PROVA]** Mande **uma** mensagem do segundo celular. Esperado: cerca de 6 segundos
  depois, o Hermes inicia uma execução e lê a mensagem pela ferramenta. Se não acordar, guarde o
  que a API do Hermes respondeu: o formato da chamada é a suposição que este passo prova.
- [ ] **6.4** Mande 4 mensagens seguidas, com 2 segundos entre elas. Esperado: **uma** execução, com
  as quatro, cerca de 6 segundos depois da última.
- [ ] **6.5** Mande uma mensagem **pelo celular pareado** (o próprio número). Esperado: não acorda.
- [ ] **6.6** Num grupo em que o número está: mensagem de um participante. Esperado: não acorda.
  Depois `adm access chat atendimento balcao ID-DO-GRUPO --read --wake` e repita: acorda.
- [ ] **6.7** Reinicie o conector depois de uma execução. Esperado: o Hermes não é acordado de novo
  pela mesma mensagem.

## Fase 7: recados entre dois Hermes

- [ ] **7.1** Um segundo Hermes, `financeiro`, pelos passos 3.1 a 3.6. Antes de qualquer ligação,
  nenhum dos dois tem ferramentas `hermes_`.
- [ ] **7.2** `adm link add atendimento financeiro --wake --both`. Esperado: o aviso de risco na
  tela, e os dois Hermes passam a ter as quatro ferramentas sem reiniciar nada.
- [ ] **7.3** `atendimento` chama `hermes_send_message` para `financeiro`. Esperado: `financeiro`
  acorda (se tiver a chave), lista o fio com um não lido, lê e responde no mesmo fio.
- [ ] **7.4** `adm link remove atendimento financeiro --both`, com um recado ainda não lido. Esperado:
  `financeiro` não lê mais esse recado; `atendimento` recebe `AR-707` ao mandar outro.

## Fase 8: o que não pode acontecer

- [ ] **8.1** Antes do passo 7.4, um Hermes pede um fio por um identificador inventado. Esperado:
  `AR-708`. Com um terceiro Hermes à mão, ele pede o fio dos outros dois pelo identificador certo:
  a mesma resposta.
- [ ] **8.2** `adm access chat atendimento balcao +55...` sem verbos (bloquear uma pessoa) e essa
  pessoa manda mensagem. Esperado: o Hermes não acorda e a leitura dessa conversa responde `AR-705`.
  Depois, `adm access chat-remove atendimento balcao +55...` desfaz o bloqueio.
- [ ] **8.3** 21 `whatsapp_send_text` em um minuto na conversa de teste. Esperado: a 21ª recusada
  com `AR-704` e o tempo de espera.
- [ ] **8.4** `adm access remove atendimento balcao` com o Hermes em sessão. Esperado: a chamada
  seguinte de leitura responde `AR-701`, e a lista de ferramentas perde as de WhatsApp. Conceda o
  acesso de novo (passo 5.4) antes de seguir.
- [ ] **8.5** `adm revoke financeiro` com esse Hermes em sessão. Esperado: o canal dele cai em menos
  de 5 segundos, e as ferramentas passam a responder o código do aparelho encerrado. O `atendimento`
  segue ligado, para a fase 9.
- [ ] **8.6** `dc logs hub` de todo o teste: nenhuma linha com a senha do banco, o convite, a chave
  do motor ou o texto de uma mensagem.

## Fase 9: quedas, cópia e fechamento

- [ ] **9.1** `time dc stop hub` com um Hermes conectado. Esperado: menos de 20 segundos, saída 0.
  `dc up -d`: os Hermes voltam a ativo sozinhos.
- [ ] **9.2 [PROVA]** Pare o hub por 6 minutos; nesse tempo mande 3 mensagens numa conversa e 3
  noutra; suba o hub e mande mais uma na primeira logo em seguida. Esperado: o Hermes é acordado
  pelas duas conversas, com todas as mensagens, em até 6 minutos.
- [ ] **9.3** Reinicie a sessão do número no WAHA e mande 3 mensagens enquanto ela volta. Esperado:
  o Hermes acorda uma vez, com as três, quando o número volta a conectado.
- [ ] **9.4** Pare o WAHA logo depois de um `whatsapp_send_text` sair. Esperado: o Hermes recebe
  `AR-713` (resultado desconhecido), nunca "motor indisponível", e não manda a mesma mensagem duas
  vezes.
- [ ] **9.5** Cópia: `dc exec postgres pg_dump -U arauto -Fc arauto > copia.dump`, e a cópia do
  volume `arauto-data` para outro lugar, separada.
- [ ] **9.6** Ensaio da pasta errada: pare o hub, suba-o com o volume `arauto-data` vazio. Esperado:
  o hub **não sobe**, por falta da chave-mestra, e `adm status` (com a pasta certa de volta) mostra
  o modo restaurado **desligado**.
- [ ] **9.7** Ensaio de restauração, só se o resto passou: restaure a cópia do passo 9.5 por cima do
  banco. Esperado: `adm status` mostra o modo restaurado, os Hermes param de ser atendidos, e cada
  um volta com um convite novo. Confira os números e os acessos antes de emitir os convites.
- [ ] **9.8** Fechamento: `adm alerts` sem alerta que você não saiba explicar; `adm audit --tail 50`
  conta a história do teste.

## O que anotar, mesmo que tudo passe

| O quê | Para quê |
|---|---|
| A versão do WAHA e a do Hermes | A lista de versões conferidas do projeto. |
| O texto da entrada `arauto` no Hermes, com e sem as variáveis de acordar | O registro automático das variáveis ainda não existe. |
| O que a API local do Hermes respondeu à chamada de acordar | O formato da chamada é suposição. |
| A resposta crua da sessão do WAHA no primeiro "conectado" (o campo `me`) | O formato do telefone no pareamento. |
| Se a mesma pessoa virou duas conversas (passo 5.6) | Decide a correção definitiva do identificador oculto. |
| O tempo de cada envio (texto, imagem, documento) | O prazo de envio do hub é de 15 segundos. |
| O tempo do `dc build` e a memória usada | O guia de instalação. |
