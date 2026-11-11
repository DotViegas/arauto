# 11 — O conector

**Estado: proposta.** O programa que roda ao lado do Hermes.

## Resumo

```
arauto-connector join     pede conexão ao hub e se registra no Hermes
arauto-connector mcp      o servidor de ferramentas que o Hermes inicia
arauto-connector status   mostra a situação, sem segredo
arauto-connector leave    pede o próprio encerramento ao hub e limpa a pasta
                          (leave --local: limpa sem o hub; é para uma pessoa)
```

Um arquivo só, sem instalar mais nada. Não abre porta de rede, não executa comando do hub e não
mostra segredo. A única coisa que descarta por informação do hub é um pedido de conexão que ainda não
recebeu credencial.

## 1. Comandos

| Comando | O que faz |
|---|---|
| `join HUB KEY INVITE` | Cria a pasta do pedido com uma identidade nova e faz a inscrição com o hub daquela chave. Se o hub aceita, copia a si mesmo para o caminho fixo `~/.arauto/arauto-connector` (por arquivo temporário e troca de nome, porque o programa antigo pode estar em execução), registra o servidor de ferramentas no Hermes e mostra o código de conferência. Termina em segundos. Com `--no-register`, para desenvolvimento, pula o registro. |
| `mcp` | O que o Hermes inicia. Fala MCP por stdio com o Hermes e mantém o canal com o hub. |
| `status` | Hub, versão, situação do aparelho, se o Hermes pode ser acordado. Lê os arquivos locais e pergunta ao líder pelo soquete local. Não abre conexão própria com o hub. |
| `leave` | Pede ao hub o encerramento do próprio aparelho. Com a resposta, apaga os arquivos e remove o registro no Hermes. |
| `leave --local` | Apaga os arquivos e o registro sem falar com o hub. Existe para quando o hub não existe mais ou trocou de chave. É para uma pessoa rodar: o aparelho continua valendo no hub até ser revogado no painel. |

### Casos do `join`

| Situação | O que acontece |
|---|---|
| Mesmo convite de novo, pedido ainda pendente | É o mesmo pedido, com a mesma identidade: o conector reconhece o convite pelo resumo guardado na pasta do pedido. O código de conferência é outro, e vale o mais recente. O hub aceita até cinco inscrições por pedido. |
| Mesmo convite de novo, pedido já aprovado | O hub responde AR-104. Nada muda: o conector só refaz o registro no Hermes, se faltar. |
| Mesmo convite de novo, pedido negado ou vencido | O hub responde AR-301, AR-302 ou AR-303. O conector descarta a pasta daquele pedido só se ela ainda não tem `credential.seal`; se tem, não descarta nada e mostra o erro. |
| O hub recusa o convite (AR-101, AR-102) | Descarta a pasta que acabou de criar para este pedido. Nada mais muda na máquina. |
| Convite novo, com um pedido antigo sem credencial parado na pasta | Descarta o pedido antigo e cria outro, com identidade nova. |
| A inscrição deu certo e o registro no Hermes falhou | Mostra AR-601. O pedido continua de pé no hub; rodar de novo tenta o registro. |
| Já existe um aparelho na pasta, do mesmo hub | É tratado como reconectar, seja qual for a linha do convite no painel. O aparelho atual continua funcionando até o novo ter a primeira credencial confirmada. Ao assumir o novo, o conector pede o encerramento do antigo, se o hub ainda não o deu como substituído, e de cada outra identidade que tenha sobrado na pasta; "já encerrado" e "não conheço este aparelho" valem como confirmação. Cada pasta só é apagada depois da resposta do hub; até lá, o líder repete o pedido a cada partida e a cada vez que o canal volta. |
| Já existe um aparelho na pasta, de outro hub | Recusa (AR-105). Só depois de `leave`; se o hub antigo não existe mais ou trocou de chave, depois de `leave --local`. Mesmo endereço com outra chave conta como outro hub. |

Ao fim, o `join` avisa o líder em execução, pelo soquete local, de que há um pedido novo. Se o
programa foi trocado por uma versão nova, pede ao líder que encerre, para o Hermes iniciá-lo de novo
já na versão nova. Isso depende de uma prova: o Hermes reinicia um servidor de ferramentas que
encerra ([12-roteiro.md](12-roteiro.md), Fase 1). Se ele não reiniciar, a alternativa é o líder
trocar a si mesmo pelo programa novo, sem fechar a ligação com o Hermes.

### Casos do `leave`

| Situação | O que acontece |
|---|---|
| O líder tem o canal aberto | O pedido vai pelo líder. Com a confirmação, apaga tudo e remove o registro. |
| Há mais de uma identidade na pasta (um reconectar em andamento) | Pede o encerramento de cada uma. Só apaga depois de todas as respostas. |
| Só há um pedido pendente | Pede ao hub que encerre o pedido, que vira negado. Com a resposta, apaga. |
| Não há canal: aparelho em quarentena, suspenso, revogado, ou conector parado | O `leave` abre uma conexão curta, prova a identidade e pede. |
| O hub responde "já encerrado", ou que não conhece o aparelho (AR-504) | Vale como confirmação: não há o que encerrar. Apaga tudo e remove o registro. |
| O hub não responde, ou não é mais o da chave guardada | **Não apaga a identidade.** Mostra AR-505: a saída ficou pendente e o aparelho precisa ser revogado no painel. Rodar de novo tenta outra vez. Se o hub não existe mais ou trocou de chave, resta a saída local, feita por uma pessoa. |

## 2. Arquivos

Tudo em `~/.arauto`, pasta que só o dono lê. A variável `ARAUTO_HOME` aponta outra pasta; nesse caso
o registro no Hermes leva `--home` nos argumentos ([15](15-mvp.md), seção 4).

```
~/.arauto/
  arauto-connector            o programa, no caminho fixo registrado no Hermes
  arauto-connector-{VERSAO}   o arquivo baixado pelo bloco; pode ser apagado depois do join
  current                     diz qual pasta de aparelho está em uso
  devices/
    {id}/                     uma pasta por aparelho ou pedido
      identity.key            SEGREDO: a identidade do aparelho
      credential.seal         SEGREDO: a credencial mais recente recebida do hub
      hub.json                endereço e chave pública do hub, resumo do convite do pedido,
                              situação local
  state.json                  últimos avisos tratados e a lista de ferramentas da última sessão
  lock                        arquivo de trava do líder
  channel.sock                soquete local entre as cópias do conector
```

- **Cada aparelho tem a pasta completa dele.** Um pedido novo nasce na própria pasta e recebe ali a
  primeira credencial. A troca de aparelho é uma única troca de nome do arquivo `current`. Na partida,
  o conector segue o que `current` aponta e acompanha, à parte, um pedido ainda não concluído. Se a
  pasta do pedido já tem credencial, ele conecta com ela: aceita pelo hub, conclui a troca do
  `current`; recusada, segue com o aparelho atual.
- **Gravação durável.** Escreve-se num temporário, força-se a gravação do arquivo, troca-se o nome e
  força-se a gravação da pasta. Só então o conector diz "gravei" ao hub.
- **O que o conector descarta por informação do hub:** só a pasta de um pedido que ainda não tem
  `credential.seal`, quando o hub recusa o convite ou diz que o pedido foi negado ou venceu. Uma
  pasta com `credential.seal` só sai por `leave`, ou depois de outro aparelho assumir e de o hub
  responder que aquela identidade foi substituída, já está encerrada ou não é conhecida. Se o hub disser que foi negado ou venceu um
  aparelho cuja pasta tem credencial, o conector não descarta nada e mostra o erro: pode ser um
  pedido aprovado que venceu, um aparelho vencido por inatividade, ou sinal de hub restaurado de
  cópia.
- **Deixe `~/.arauto` fora das cópias de segurança do volume do Hermes.** Restaurar uma cópia
  antiga dessa pasta leva o aparelho à quarentena.

## 3. Um líder por Hermes

O Hermes inicia o servidor de ferramentas como processo filho, e mais de um processo do Hermes pode
fazer isso. Se cada cópia abrisse o próprio canal, o hub veria conexões duplas de um Hermes saudável.

```
 cópia 1  pega a trava  ->  LÍDER: abre o canal com o hub e o soquete local
 cópia 2  não pega      ->  REPASSADORA: manda as chamadas ao líder pelo soquete
 cópia 3  não pega      ->  REPASSADORA
 líder morre            ->  o sistema solta a trava; a próxima cópia assume
```

- Só o líder segura o canal, troca a credencial, acompanha um pedido novo e recebe avisos.
- O líder encerra uma tentativa de conexão antes de abrir outra.
- Para o hub existe exatamente uma conexão de canal por aparelho. Por isso "duas conexões" volta a
  significar cópia. As conexões curtas do `join` e do `leave` não contam.
- O soquete local só aceita processo do mesmo usuário.
- Outro processo local do mesmo usuário consegue usar o soquete. Esse é o limite do modo rápido; o
  registro de cada chamada fica na auditoria do hub.
- A pasta do conector não pode ser compartilhada entre dois Hermes, e o sistema de arquivos precisa
  suportar trava. Sem isso, duas cópias viram líder, o hub as trata como cópia e o aparelho vai para
  a quarentena.
- Uma chamada em andamento quando o líder morre volta com erro: AR-203 se ela não chegou a sair,
  AR-207 se saiu e ficou sem resposta.
- Quando o hub manda o líder parar de usar o canal (quarentena, suspenso, revogado, incompatível), o
  líder continua vivo: responde às ferramentas com o erro e atende o `join` e o `leave` pelo soquete
  local.

O formato das mensagens no soquete local está em
[14-protocolo-do-canal.md](14-protocolo-do-canal.md), seção 11.

## 4. Registro no Hermes

O conector se registra pelo comando oficial do Hermes:

```
hermes mcp add arauto --command ~/.arauto/arauto-connector --args mcp
```

- A entrada não contém segredo, e é a única mudança que fica na configuração do Hermes.
- O Hermes confere toda entrada de servidor de ferramentas ao salvar e ao iniciar, e recusa as que
  parecem ataque. O conector segue o formato que o Hermes aceita: um programa com um argumento, sem
  interpretador de comandos. O projeto não tenta contornar essa conferência.
- Se o Hermes pedir aprovação humana para rodar o bloco, quem aprova é o operador. O fluxo funciona
  com ou sem esse pedido.
- O Hermes passa a iniciar o conector sozinho, em menos de um minuto, sem reiniciar.

## 5. Partida e encerramento

**Partida com o hub fora do ar.** O conector não morre: responde ao Hermes com a lista de
ferramentas da última sessão e devolve o erro AR-203 nas chamadas. Tenta o hub de novo com espera
crescente e sorteada, de 1 a 60 segundos.

**Encerramento.** O Hermes fecha a entrada do conector e espera 2 segundos antes de encerrá-lo à
força. Nesse tempo o conector fecha o canal em ordem. Não há nada a salvar na saída: a credencial já
está em disco desde a última troca.

## 6. Acordar o Hermes

Ao receber um aviso, o líder chama a API local do Hermes com a frase fixa
([03-arquitetura.md](03-arquitetura.md), seção 7) e só então confirma ao hub. Um aviso repetido é
descartado pelo identificador. O conector aceita no máximo 30 acordadas por minuto e 300 por hora,
somando os tipos, para um hub com defeito não disparar execuções sem parar. Antes de montar a frase
ele confere o formato de cada campo do aviso ([14](14-protocolo-do-canal.md), seção 5.3).

**A chave da API local.** A API do Hermes exige uma chave, que fica no arquivo de segredos do
Hermes. O conector, como processo filho, não a recebe do ambiente. Há três caminhos, e a ordem está
decidida ([decisoes/0023](decisoes/0023-chave-da-api-local.md)): tenta-se o primeiro; se o teste da
Fase 1 mostrar que ele não funciona, vale o segundo; sem chave, fica o terceiro.

| Caminho | Prós e contras |
|---|---|
| A entrada do servidor referencia a variável do Hermes | O próprio Hermes entrega a chave ao filho. Nada é copiado. Depende de o Hermes aceitar a referência nesse campo. |
| O conector lê a chave do arquivo de segredos do Hermes | Funciona sempre. Um programa de fora lendo esse arquivo precisa ser dito a quem instala. |
| Sem chave, sem acordar | O Hermes usa as ferramentas quando pedem e nunca é acordado. O painel mostra "Pode ser acordado: não". |

Endereço, corpo da chamada e o uso do identificador de repetição da API são entregas da Fase 1.

## 7. Versões e distribuição

- Cada versão do conector é publicada nas versões do projeto, assinada com a chave de versões, com o
  resumo SHA-256 ao lado.
- Cada versão do hub traz os conectores compatíveis com ela e os serve no endereço de download.
- **Em toda instalação pelo bloco**, a primeira e cada reconectar, a conferência é o resumo SHA-256
  que vem nele. Quem quiser uma segunda fonte compara com o resumo publicado nas versões do projeto.
- A assinatura serve para o hub conferir os conectores que distribui e para quem quiser conferir por
  fora. Se um dia o conector se atualizar sozinho, só aceitará versão assinada e de número maior que
  o seu.
- O hub exige uma versão mínima. Abaixo dela, toda ferramenta responde AR-401.
- A primeira mensagem do canal nunca muda de formato, para um conector muito antigo ainda receber o
  AR-401 em vez de um erro mudo. O que mais fica congelado está em
  [14-protocolo-do-canal.md](14-protocolo-do-canal.md), seção 5.1.
- Na primeira versão do projeto, atualizar é **reconectar**: bloco novo, acessos mantidos. Não há
  atualização empurrada pelo hub.

## 8. Catálogo de mensagens

Toda mensagem é de texto fixo, sem acento, e diz o que houve e quem resolve. Nenhuma traz comando
para executar.

| Código | Quando |
|---|---|
| AR-101 | Convite não aceito: vencido, cancelado, inexistente ou já usado. |
| AR-102 | O convite já foi usado por outro aparelho. Nada foi instalado aqui. |
| AR-104 | Este pedido já foi aprovado. Nada a fazer. |
| AR-105 | Já existe nesta pasta um aparelho de outro hub. É preciso sair dele antes; se o hub antigo não existe mais ou trocou de chave, uma pessoa faz a saída local. |
| AR-201 | Sem conexão com o hub, com a causa: nome não resolvido, tempo esgotado, conexão recusada. |
| AR-202 | O hub não é o da chave que veio no bloco. Conexão recusada. |
| AR-203 | Canal fora do ar: a chamada não saiu, e pode ser repetida. O conector continua tentando. |
| AR-204 | O hub fechou a conexão: erro de protocolo ou limite. O conector tenta de novo. |
| AR-205 | O hub está parando ou sem recursos. O conector tenta de novo em instantes. |
| AR-206 | Esta tentativa foi trocada por outra mais nova do mesmo aparelho. |
| AR-207 | A chamada saiu e o canal caiu antes da resposta: o resultado é desconhecido. Confira antes de repetir. |
| AR-208 | Conexão dada por morta: outra conexão do mesmo aparelho entrou no lugar. O conector reconecta na hora. |
| AR-301 | O pedido foi negado. |
| AR-302 | O pedido venceu sem aprovação: ninguém confirmou o código no prazo, ou o hub encerrou o pedido. |
| AR-303 | O pedido foi aprovado, mas venceu antes de o conector concluir. O admin precisa reconectar. |
| AR-401 | Conector desatualizado. O admin precisa reconectar este Hermes pelo painel. |
| AR-402 | O hub é mais antigo que este conector. Quem instala precisa atualizar o hub. |
| AR-501 | Aparelho em quarentena: o hub deixou de confiar na credencial. O admin precisa reconectar este Hermes pelo painel. |
| AR-502 | Aparelho revogado. |
| AR-503 | Aparelho substituído por outro. |
| AR-504 | O hub não reconhece este aparelho. Nada foi apagado. |
| AR-505 | Saída pendente: o hub não respondeu. O aparelho precisa ser revogado no painel. |
| AR-506 | Aparelho suspenso: o hub voltou de uma cópia de segurança. O admin precisa reconectar este Hermes pelo painel. |
| AR-507 | Aparelho vencido por inatividade. O admin precisa reconectar este Hermes. |
| AR-601 | O registro no Hermes falhou ou foi recusado. O pedido continua de pé; rode o comando de novo. |
| AR-602 | A API local do Hermes não responde ou recusa a chave. |
| AR-701 | Falta o verbo necessário para esta chamada. |
| AR-702 | Número indisponível: desconectado do WhatsApp ou aguardando confirmação do admin. |
| AR-703 | Motor indisponível. |
| AR-704 | Teto de envio atingido. |
| AR-705 | Conversa não encontrada. É a mesma resposta para conversa que não existe e para conversa fora do escopo. |
| AR-706 | Teto de recados atingido. A mensagem diz em quanto tempo ele volta; insistir não adianta. |
| AR-707 | Não há par com esse nome para o qual este Hermes possa mandar. Só o admin muda isso. |
| AR-708 | Fio não encontrado. É a mesma resposta para fio que não existe e para fio de outros. |
| AR-709 | Grande demais: peça menos itens ou mande um conteúdo menor. |
| AR-710 | Chamadas demais ao mesmo tempo. Tente de novo em instantes. |
| AR-711 | Argumento inválido. |

A frase de cada código, em inglês, fica compilada no conector.

## 9. Modo blindado (previsto)

No modo rápido, descrito aqui, a identidade e a credencial ficam onde o terminal da LLM alcança. No
modo blindado, uma pessoa instala o conector como serviço separado, com outro usuário ou em outro
container, e o Hermes fala com ele por um endereço local. A LLM deixa de alcançar os arquivos.

Não é da primeira versão. O painel mostra, por Hermes, o limite do modo em uso: "Proteção básica: a
LLM alcança o arquivo da credencial" ou "Proteção blindada".

## 10. Plataformas

| Plataforma | Quando |
|---|---|
| Linux x86_64 | Primeiro. |
| Linux arm64 | Depois. |
| Windows | Só para desenvolvimento ([decisão 0036](decisoes/0036-windows-so-para-desenvolvimento.md)). |
| macOS | Depois. |
