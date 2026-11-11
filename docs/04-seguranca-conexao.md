# 04 — Segurança da conexão

**Estado: proposta v0.4**, a validar com um protótipo antes de implementar
([12-roteiro.md](12-roteiro.md), Fase 1). Especifica como um Hermes entra, volta e sai do hub. Os
termos estão no [glossário](glossario.md).

Este desenho já passou por várias rodadas de leitura crítica, e cada uma achou falhas que a anterior
não viu. A v0.4 reúne as correções da terceira rodada em diante: o hub restaurado de cópia, o
reconectar, o sair, os comandos repetidos, o código de conferência e a ordem de gravação do lado do
hub. Ele só merece confiança depois do protótipo com rodadas sorteadas.

## Resumo

```
CONVITE            15 minutos, 1 uso     só serve para PEDIR conexão
CÓDIGO             6 dígitos             o admin DIGITA o que o Hermes respondeu
IDENTIDADE         nasce no Hermes       a parte privada nunca sai da máquina
CREDENCIAL LACRADA nasce no hub          para usar o hub é preciso trocá-la por outra
```

| Pergunta | Resposta |
|---|---|
| O que passa pelo chat? | Só o convite, que morre no primeiro uso, e o código de conferência, que não é segredo. |
| Quem cria os segredos? | A identidade nasce no conector. A credencial lacrada nasce na caixa preta do hub. |
| Alguma pessoa vê um segredo duradouro? | Não. Nenhum aparece em tela, em registro ou em resposta de ferramenta. |
| E se copiarem a credencial? | Quem usa a cópia é descoberto quando o aparelho verdadeiro volta a apresentar a dele. Os limites estão na seção 6. |
| O que o desenho não resolve? | Conta de admin tomada, servidor do hub invadido e motor exposto. Ver a seção 6 e [05](05-modelo-de-ameacas.md). |

## 1. Peças usadas

Nada de criptografia inventada. Cada passo usa uma peça conhecida.

| Uso | Peça |
|---|---|
| Sorteio de todo segredo | Gerador do sistema operacional. Nada vem de data, hora, nome ou contador. |
| Canal conector ↔ hub | Noise `XK_25519_ChaChaPoly_BLAKE2s`, por dentro do websocket. Uma suíte fixa, sem modo de reserva. Autenticação mútua e sigilo futuro. |
| Identidade do aparelho e chave do hub | X25519. |
| Convite | 32 bytes sorteados. O hub guarda só o resumo SHA-256. |
| Código de conferência | HKDF-SHA-256 sobre o resumo do aperto de mãos e um número sorteado pelo hub, reduzido a seis dígitos sem viés. |
| Lacre da credencial | AEAD XChaCha20-Poly1305, com a chave da organização. |
| Assinatura das versões do conector | Ed25519. |
| Login do painel | Passkey (WebAuthn). |
| Troca da credencial | Rotação com detecção de reuso, o padrão da renovação de credenciais do OAuth (RFC 9700). |

O canal é cifrado por dentro do websocket para não depender de quem termina o TLS. Em instalações
reais o TLS termina num proxy que não é do projeto, e numa rede interna ele pode nem existir. Com o
Noise, a segurança do canal é a mesma nos dois casos.

## 2. Chaves do hub

| Chave | Onde fica | Para que serve |
|---|---|---|
| **Mestra** | Arquivo fora do banco e fora da cópia de segurança do banco. | Cifra as outras chaves e os segredos guardados no banco. |
| **Da organização** | No banco, cifrada pela mestra. | Lacra as credenciais e cifra o arquivo de mensagens. |
| **Do hub** (X25519) | No banco, cifrada pela mestra. | É por ela que o conector reconhece o hub. A parte pública vai em todo bloco. |

Trocar ou perder a chave do hub obriga a conectar todos os Hermes de novo. É um custo aceito:
conectar é justamente o que o projeto torna barato. Antes, cada Hermes precisa de uma saída local,
feita por uma pessoa (C10): o conector não troca de hub sozinho.

A chave de versões é do projeto, não do hub. Ela assina cada versão publicada do conector. Na
primeira versão do projeto não há atualização automática: toda instalação passa pelo bloco, e a
conferência é o resumo que vem nele ([11-conector.md](11-conector.md), seção 7).

## 3. Fluxo

### C1. Criar o convite

O admin clica "Conectar Hermes" e dá o nome. O hub sorteia 32 bytes: é o convite (`arc1` + base32).
Guarda **só o resumo SHA-256** dele, com validade de 15 minutos. O convite aparece uma única vez,
dentro do bloco.

Quem lê o banco não consegue usar o convite. Quando ele vence, o resumo é apagado: um relógio
atrasado não o faz valer de novo.

### C2. O bloco

O bloco traz o nome do Hermes, o endereço de download do conector numa versão exata, o resumo SHA-256
dessa versão, o endereço do hub, a chave pública do hub e o convite. Não usa `curl | sh`. O texto
proposto está em [02-como-funciona.md](02-como-funciona.md).

O convite vai como argumento do conector. Não aparece em endereço, então não fica em registro de
servidor nem é lido por pré-visualização. O endereço de download é público e não carrega segredo.

### C3. Inscrição

O conector:

1. Sorteia a identidade do aparelho e a grava, numa pasta só daquele pedido, antes de falar com o
   hub. **Convite novo, identidade nova:** uma identidade só é reaproveitada quando o mesmo convite é
   apresentado de novo. O conector reconhece o mesmo convite pelo resumo dele, guardado na pasta do
   pedido.
2. Abre o aperto de mãos Noise XK. A chave estática do conector é a **identidade do aparelho**; a do
   hub é a que veio no bloco. Um hub falso não completa o aperto de mãos.
3. Envia, já cifrado para o hub: o convite e os dados do aparelho (nome da máquina, versão do
   conector, versão do Hermes, sistema).

O hub procura o resumo do convite e decide:

| Situação | Resposta |
|---|---|
| Convite emitido e dentro do prazo | Cria o aparelho como **pendente**, marca o convite como usado e responde com o prazo de confirmação. |
| Convite usado, mesma identidade, pedido ainda pendente | É o mesmo pedido. Não muda prazo nem tentativas. Passa a valer o código desta inscrição (C4). Na sexta inscrição o pedido é negado. |
| Convite usado, mesma identidade, pedido já resolvido | Diz a situação exata a quem provou ser o mesmo: já aprovado (AR-104), negado (AR-301) ou vencido (AR-302, AR-303). Não muda nada. |
| Convite usado, outra identidade, pedido ainda pendente | Recusa (AR-102) e **alerta o painel**: o bloco foi parar em outro lugar. O primeiro pedido não é tocado. |
| Qualquer outro caso | Recusa única (AR-101), sem dizer por quê. Só gera registro na auditoria. |

O convite só viaja cifrado, dentro de um canal que só o hub verdadeiro consegue abrir. No mesmo aperto
de mãos o conector prova que tem a identidade que está inscrevendo.

Um convite encerrado **nunca derruba um Hermes conectado**.

**Um pedido por Hermes.** Só existe um pedido em andamento por Hermes. Gerar um convite novo para um
Hermes cancela o convite emitido e encerra o pedido que estiver pendente, ou aprovado ainda sem
credencial confirmada. Conta como pedido em andamento também o que foi à quarentena antes de
confirmar a primeira credencial: o convite novo o encerra do mesmo jeito. O aparelho em uso não é
tocado.

### C4. Código de conferência

Os dois lados calculam, cada um por si, seis dígitos a partir de duas coisas: o resumo do aperto de
mãos e um número que o hub sorteia e só envia, pelo canal já cifrado, depois da última mensagem do
aperto de mãos. Sem esse número, quem se inscreve conseguiria escolher o próprio código, refazendo a
última mensagem até dar o que quer. O hub **nunca envia nem mostra** o código.

- O conector mostra o código e termina.
- O Hermes diz o código no canal de comando.
- O admin digita o código no painel.

O painel só aprova se o que foi digitado bater com o pedido pendente **daquele Hermes**, que é um só
(C3). Cinco erros encerram o pedido. O prazo é de 10 minutos a partir da primeira inscrição.

Se o comando do bloco rodar de novo no mesmo Hermes, há um aperto de mãos novo e, com ele, um código
novo. **Vale o mais recente.** O conector diz, na resposta, que o código anterior deixou de valer. O
hub aceita até cinco inscrições por pedido: com o sorteio do hub em cada uma, repetir não ajuda a
chegar a um código escolhido.

Por que digitar em vez de comparar e clicar: quem obtiver o convite antes do uso consegue criar um
pedido com o nome certo. Se bastasse um clique, esse pedido seria aprovado. Digitando, só é aprovado
o pedido cujo código o Hermes verdadeiro disse.

O que o aparelho declara (nome da máquina, versão) aparece na tela como "informado pelo aparelho, não
verificado". Não decide nada.

### C5. Aprovação

Com o código certo, o aparelho passa a **aprovado**. Numa primeira conexão, o Hermes fica **sem
acesso a número nenhum**: os acessos são dados depois, na matriz ([08](08-permissoes.md)). Num
reconectar, os acessos que o Hermes já tinha passam a valer para o aparelho novo assim que ele
confirma a primeira credencial (C10).

Um aparelho aprovado está na **geração zero**: ainda não tem credencial **confirmada**. Continua na
geração zero mesmo depois de o hub emitir a primeira, até o "gravei" dela. Ele recebe a primeira
pela mesma troca de todas as outras (C7).

### C6. Registro no Hermes e espera

Ao fim da inscrição o conector se registra no Hermes como servidor de ferramentas, pelo comando
oficial (`hermes mcp add`). A entrada não contém segredo: só o caminho do programa e o argumento
`mcp`. É a única mudança que fica na configuração do Hermes.

O Hermes inicia o conector em menos de um minuto. Enquanto o aparelho está pendente, o conector abre
o canal **em espera**: prova a identidade, não tem ferramenta nenhuma de WhatsApp e aguarda.

| O que acontece com o pedido | O que o conector faz |
|---|---|
| Aprovado | Faz a primeira troca ali mesmo (C7). |
| Negado ou vencido | Mostra o código do erro. Se a pasta **daquele pedido** ainda não tem credencial, descarta a pasta e, se não há outro aparelho, remove o registro. |
| Aparece uma segunda conexão em espera com a mesma identidade, e a primeira responde | O hub nega o pedido e alerta: a identidade foi copiada antes da aprovação. |

**O conector nunca descarta, por informação do hub, uma pasta que já tem credencial.** Só descarta
um pedido que ainda não recebeu nenhuma. As pastas que sobram quando outro aparelho assume são caso
à parte: saem por decisão do conector, depois de o novo assumir e de o hub responder (C10).

Se a inscrição deu certo e o registro no Hermes falhou (AR-601), o pedido continua de pé no hub.
Repetir o comando tenta o registro de novo.

### C7. A troca da credencial

Toda conexão de canal de um aparelho aprovado ou ativo começa por uma troca. A sessão do canal só
existe depois dela. As conexões curtas do `join` e do `leave` não são de canal (C9).

```
conector -> hub     aperto de mãos (prova a identidade) + a credencial que tem
hub                 confere que quem apresentou continua na linha
hub                 grava a nova como pendente
hub -> conector     credencial NOVA
conector            grava a nova, de forma durável
conector -> hub     "gravei"
hub                 grava: a nova passa a valer; a anterior morre
                    só agora a sessão serve ferramentas e avisos
```

**Regras**

1. Toda credencial emitida tem uma geração nova e um identificador sorteado dentro do lacre. O lacre
   amarra a organização, a identidade do aparelho e a geração.
2. O hub guarda, por aparelho, a geração e o resumo da **vigente** e da **pendente**, e a maior
   geração já emitida, inclusive depois de o aparelho ser encerrado. Compara o resumo, não só o
   número: depois de uma restauração do hub, um número de geração pode se repetir.
3. O hub só julga a credencial de quem provou a identidade no aperto de mãos **e** respondeu a uma
   conferência na mesma conexão. Uma tentativa abandonada, que chega atrasada pela rede, não responde
   e é descartada sem consequência. O conector encerra uma tentativa antes de abrir outra.
4. **A sessão só serve ferramentas e avisos depois do "gravei".** Quem quer usar o hub tem de
   confirmar a troca, e confirmar mata a credencial anterior. É isso que faz o uso de uma cópia
   aparecer quando o aparelho verdadeiro volta. Os limites estão na seção 6.
5. O conector só diz "gravei" depois de a gravação estar garantida em disco, no arquivo e na pasta.
6. O hub só envia a credencial nova depois de gravar, de forma durável, a pendente e a maior geração.
   Só abre a sessão depois de gravar a promoção.
7. Um "gravei" que não se refere à pendente é recusado, e a conexão é fechada.
8. **Depois da geração zero, o hub nunca emite credencial só pela identidade.**

**O que o hub faz com o que o conector apresenta**

Vale para aparelho aprovado ou ativo. Em qualquer outro estado o hub responde a situação do
aparelho e não troca nada. Duas conferências valem em todos os estados:

- Um lacre válido de geração maior do que o banco conhece é assunto de C11: pode ser prova de
  restauração.
- Um aparelho revogado, substituído ou vencido por inatividade que apresenta um lacre válido **mais
  antigo** do que a última vigente e a última pendente gera o alerta "reuso depois do
  encerramento": alguém usou uma cópia, e o aparelho foi encerrado antes de o verdadeiro voltar.

| O conector apresenta | O hub |
|---|---|
| A vigente (ou nada, se o aparelho está na geração zero) | Aceita e emite uma nova, que passa a ser a pendente. Se já havia uma pendente, ela é descartada. |
| A pendente | Aceita, promove a pendente a vigente, e a vigente anterior morre. Emite outra. |
| Qualquer outra já emitida, ou nada quando o aparelho já passou da geração zero | Quarentena e alerta, com o motivo "reuso" (C9). |
| Um lacre que não abre, ou que é de outro aparelho | Quarentena e alerta, com o motivo "credencial ilegível". |
| Um lacre válido, de geração maior do que todas as que o hub conhece | É prova de que o hub voltou de uma cópia antiga. Liga o modo restaurado (C11). |

A troca é em duas fases para uma queda no meio não trancar o Hermes do lado de fora:

| Onde caiu | O que o conector tem | Na volta |
|---|---|---|
| Antes de gravar a nova | A vigente | Apresenta a vigente e recebe outra. |
| Na primeira troca, antes de gravar a primeira | Nada | Apresenta nada. O aparelho continua na geração zero e recebe outra. |
| Depois de gravar, antes de o hub ouvir o "gravei" | A pendente | Apresenta a pendente, e o hub a promove. |

Uma conexão que termina sem "gravei" fica registrada. Três em 24 horas do mesmo aparelho geram
alerta, mesmo com conexões confirmadas no meio: pode ser rede ruim, pode ser alguém testando uma
cópia.

**A troca de 24 horas.** A cada 24 horas o hub repete a troca dentro da sessão aberta, sem derrubar o
canal. Vale como apresentada a vigente. A sessão continua servindo enquanto o conector grava. O hub
não emite outra enquanto houver uma pendente sem confirmação: se o "gravei" não chegar em 60
segundos, ele fecha a conexão, e na volta valem as linhas de sempre.

Um Hermes que perde o arquivo da credencial e mantém a identidade cai na terceira linha da tabela:
quarentena, e a saída é reconectar.

### C8. Uso

Pelo canal passam chamadas de ferramenta (Hermes → hub) e avisos (hub → Hermes).

- **Autorização a cada chamada.** O hub confere o estado do aparelho e o acesso (Hermes, número,
  verbo, conversa) em toda chamada e em todo aviso. Tirar um acesso vale na chamada seguinte.
- **O hub não manda texto livre.** O aviso é um ponteiro: qual número, qual conversa, quantas
  mensagens. A frase que acorda o Hermes é fixa e fica no conector.
- **As descrições das ferramentas ficam no conector.** O hub só diz quais nomes estão liberados.
- **O conector não executa comando do hub.** Não existe mensagem do hub que rode algo na máquina do
  Hermes, nem que apague um aparelho que já funcionou. A primeira versão não tem atualização
  empurrada pelo hub.
- **A versão é conferida logo depois do aperto de mãos, antes da troca.** Um conector abaixo da
  versão mínima recebe AR-401 e nada mais. A exceção é o pedido de encerramento (`leave`), aceito em
  qualquer versão. Mensagem de quem ainda não provou a identidade não muda estado nenhum no hub.

### C9. Quando o hub deixa de confiar na credencial

Há um só canal por aparelho ([11-conector.md](11-conector.md) explica como isso é garantido do lado
do Hermes). O canal é a conexão que o líder mantém, em espera ou em sessão. As conexões curtas do
`join` (inscrição) e do `leave` (pedido de encerramento) provam a identidade, levam um único pedido
e fecham: não apresentam credencial, não viram sessão e não contam como conexão dupla.

Com isso, dois sinais significam cópia:

| Sinal | Resposta |
|---|---|
| **Reuso:** chega uma credencial que já foi trocada, ou nenhuma quando deveria haver | Quarentena e alerta. |
| **Conexão dupla:** chega uma apresentação que seria aceita (uma credencial, ou nada na geração zero) enquanto outra conexão do mesmo aparelho responde | Recusa a segunda, quarentena e alerta. |

A quarentena também vem sem cópia, quando o arquivo da credencial se perde ou se estraga.

Antes de tratar como conexão dupla, o hub testa a conexão antiga. Se ela não responder em 3 segundos,
é dada por morta: a antiga é fechada e a nova segue. **Toda vez que isso acontece com uma sessão que
estava servindo, o painel alerta**: "a conexão de {nome} foi tomada por outra, vinda de {endereço}".
O teste e o batimento viajam por dentro do canal cifrado: um proxy no caminho não responde por eles.

O conector verdadeiro percebe que foi dado por morto de dois jeitos: pelo fechamento que o hub envia,
ou, se a rede dele está sendo segurada, pela falta de batimento, em até 90 segundos. Nos dois casos
ele **reconecta na hora**. Se alguém tomou o lugar dele, aparece nessa volta: como reuso, se o outro
já trocou a credencial, ou como conexão dupla, se ainda não trocou.

Portanto: com o Hermes verdadeiro ligado, em versão que o hub aceita e alcançando o hub, a cópia é
barrada em segundos; no pior caso, quando a conexão dele estava muda sem ele saber, em até 90
segundos. Não é assim quando quem tem a cópia também segura a rede do verdadeiro: ela é descoberta
quando ele voltar a falar com o hub. E quem segura a rede pode esperar a conexão do verdadeiro cair
por falta de batimento antes de entrar: aí não há conexão tomada, e o painel mostra só uma volta
vinda de outro endereço. Também não é assim quando o conector verdadeiro ficou abaixo da versão
mínima: ele para de usar o canal e não apresenta mais a credencial dele (seção 6, item 3).

Rede ou país novo gera alerta, nunca bloqueio: endereços residenciais e de operadora mudam. Esse
alerta e o das trocas sem confirmação (C7) podem ser provocados por quem controla a rede: sozinhos,
não são motivo para revogar.

### C10. Quarentena, revogar, reconectar e sair

**Quarentena.** O hub deixou de confiar na credencial daquele aparelho. Canal fechado, chamadas
negadas, avisos retidos. Não existe "liberar": depois de um sinal de cópia não há como saber qual
lado é o verdadeiro. A saída é reconectar ou revogar.

**Revogar.** Revogar um Hermes encerra tudo o que é dele: o aparelho, o pedido em andamento (pendente
ou aprovado) e o convite emitido. O aparelho passa a revogado e a linha é mantida como registro. O
canal cai em segundos e a chamada seguinte já é negada. O conector para de usar o canal e mostra
AR-502. Ele não apaga nada; a limpeza é feita por `leave` ou pelo próximo `join`.

**Reconectar.** Convite novo para o mesmo Hermes, com um aparelho antigo ainda na pasta:

1. Cada aparelho tem a sua pasta, com identidade, credencial e dados do hub. O `join` cria a pasta
   do pedido novo, sem tocar na do aparelho atual.
2. Enquanto o pedido novo não é aprovado, o aparelho atual segue como está. Se o pedido for negado ou
   vencer, só a pasta do pedido é descartada.
3. Ao aprovar, o painel mostra o último uso do aparelho atual, com data e endereço, e pergunta se o
   admin o reconhece. Se o Hermes esteve desligado e houve uso, alguém usou uma cópia. Se o admin
   não reconhece, sai o alerta de cópia e a aprovação segue. O aparelho antigo, se estava ativo, vai
   para a quarentena ali mesmo; se já estava suspenso ou em quarentena, fica como está. Quando o
   aparelho anterior venceu por inatividade, o painel mostra o último uso dele do mesmo jeito, e ele
   segue vencido.
4. Confirmada a primeira credencial do aparelho novo, o hub o passa a ativo e passa o antigo a
   **substituído** na mesma gravação, venha ele de ativo, de suspenso ou de quarentena. O conector
   passa a usar a pasta nova numa única troca de nome.
5. **Uma máquina, um aparelho.** Se o convite era de outra linha do painel, o hub não tem aparelho
   antigo para substituir. Por isso, ao assumir o aparelho novo, o conector pede o encerramento do
   antigo, a não ser que o hub já o tenha dado como substituído, e de cada outra identidade que
   tenha sobrado na pasta, como um pedido encerrado que já tinha credencial. "Já encerrado" e "não
   conheço este aparelho" valem como confirmação. Cada pasta só é apagada depois da resposta do
   hub; até lá, o líder repete o pedido a cada partida e a cada vez que o canal volta. O alerta
   desse encerramento mostra o último uso do aparelho antigo, como no passo 3. Quando ninguém pede
   o encerramento, o aparelho antigo segue ativo no hub sem máquina, até o admin revogar: é o caso
   da saída local e o da pasta antiga perdida (máquina refeita) com convite de outra linha do
   painel.

Os acessos são do Hermes, não do aparelho, e passam a valer para o aparelho novo.

**Sair.** O comando `leave` pede ao hub o encerramento do próprio aparelho, espera a resposta e só
então apaga os arquivos e remove o registro.

- O pedido vale para **cada identidade** que houver na pasta: o aparelho atual e, se houver, um
  pedido em andamento. O conector só apaga depois de todas as respostas.
- Se o líder tem o canal aberto, o pedido vai por ele. Se não tem (aparelho em quarentena, suspenso,
  revogado, ou conector parado), o `leave` abre uma conexão curta, prova a identidade e pede.
- "Já encerrado" e "não conheço este aparelho" valem como confirmação.
- Se o hub não responde, o conector **não apaga a identidade**: avisa com AR-505 que a saída ficou
  pendente e que o aparelho precisa ser revogado no painel.
- **Todo encerramento pedido pelo próprio aparelho gera alerta no painel**, com o endereço de onde
  veio e o último uso do aparelho. Sem isso, quem usou uma cópia poderia encerrar o aparelho para
  apagar o rastro.

**Saída local.** `leave --local` apaga os arquivos e o registro sem falar com o hub. Existe para
quando o hub não existe mais ou trocou de chave. É para uma pessoa rodar: o aparelho continua
valendo no hub até ser revogado no painel.

Quem tem a identidade de um aparelho consegue encerrá-lo. É negação de serviço, do mesmo tipo de
apresentar uma cópia antiga, e o alerta mostra de onde veio.

### C11. Hub restaurado de cópia

Um hub que volta de uma cópia de segurança antiga esquece o que aconteceu depois dela: aprovações,
trocas de credencial, quarentenas, reconexões, revogações e mudanças de acesso. Para ele, uma
credencial que já tinha morrido volta a valer e um aparelho revogado volta a estar ativo.

**Como percebe.** De dois jeitos:

- Uma **marca fora do banco**, ao lado da chave-mestra: o número do último evento de segurança da
  auditoria. Conta todo evento que muda quem pode o quê: convite usado ou cancelado, aprovação e
  negação, troca de credencial confirmada, quarentena, reconexão, revogação, mudança de acesso e
  mudança de admin. A marca só avança depois de o evento estar garantido em disco no banco: uma
  queda entre os dois deixa a marca um evento atrás. Isso não acusa restauração à toa; o custo é não
  perceber uma restauração que perca só aquele último evento. Na partida, banco atrás da marca, ou
  banco com dados e marca ausente, quer dizer que ele voltou de uma cópia.
- Um **lacre válido que o banco não explica**: de geração maior do que todas as que ele conhece
  daquele aparelho, ou de um aparelho que ele não conhece. Só o hub cria credenciais, e ele grava a
  geração antes de enviar (C7, regra 6). Lacre válido que o banco desconhece é banco atrasado. Não
  vale para aparelho já marcado por uma restauração anterior (abaixo).

Se o servidor inteiro for restaurado de uma imagem, a marca volta junto e só o segundo jeito
funciona: o hub só percebe quando conecta um aparelho que trocou de credencial, ou que foi conectado,
depois da imagem.

**O que faz: liga o modo restaurado.**

- **Todos os aparelhos ativos ficam suspensos**, e as sessões abertas são fechadas. O hub não sabe
  mais quais credenciais valem, quais aparelhos foram trocados e quais foram revogados. Um aparelho
  suspenso recebe AR-506, para de usar o canal e não apaga nada.
- **Convites emitidos e pedidos em andamento são encerrados**: os pendentes e os aprovados ainda sem
  credencial confirmada. Um pedido que já estava em quarentena fica como está. Um convite já usado
  depois da cópia voltaria a valer pelo resto do prazo.
- **Todo aparelho que já existia no banco fica marcado**, em qualquer estado. Um aparelho marcado
  não serve mais de prova de restauração: um lacre de geração maior vindo dele não liga o modo de
  novo, só gera registro. O resto continua valendo para ele: o AR-504 com a listagem (abaixo) e o
  alerta de reuso depois do encerramento (C7).
- **A numeração da auditoria continua a partir da marca.** O banco volta a ficar à frente dela, e
  reiniciar o hub não acusa a mesma restauração de novo. Ligar o modo é uma única gravação no banco
  (suspensões, encerramentos, marcações e o primeiro evento novo), e só depois a marca avança. Se a
  marca estava ausente, a numeração segue do último número do banco e o hub recria a marca.
- **Não existe "reativar".** Cada Hermes sai do suspenso por **reconectar**, com convite novo, ou é
  **revogado**. Reconectar cria identidade e credencial novas, e com isso deixa de servir qualquer
  cópia dos arquivos antigos do conector, tirada antes ou depois da cópia de segurança do hub.
- O painel mostra, em qualquer tela: "O hub voltou de uma cópia de {data}. O que foi aprovado,
  revogado, reconectado ou mudado nos acessos depois disso se perdeu. Reconecte cada Hermes e confira
  os acessos e a lista de admins."
- Um aparelho que o banco não conhece, ou que no banco era um pedido encerrado pela restauração, e
  que apresenta um lacre válido, é um Hermes conectado ou reconectado depois da cópia. Recebe AR-504
  e **não apaga nada**; o painel o lista, com a hora, o endereço e o que ele declara, e guarda a
  identidade dele. O caminho dele também é reconectar.
- O modo termina quando não resta aparelho suspenso.

**Restauração nova com o modo ligado.** Banco atrás da marca, ou banco com dados e marca ausente,
numa partida, ou lacre de geração maior vindo de um aparelho criado depois do início do modo, é
outra restauração: o modo recomeça e vale para todos os aparelhos, inclusive os criados durante o
anterior. Um lacre de geração maior vindo de aparelho marcado, ou um lacre de desconhecido já
listado, não recomeça o modo: só gera registro.

Mesmo com a cópia contínua do banco de controle ([03](03-arquitetura.md), seção 8), em que a perda é
de minutos, todos reconectam: o hub não tem como saber o que perdeu. É um custo parecido com o de
trocar a chave do hub, aceito pelo mesmo motivo.

**Em aberto.** Depois de terminado o modo, ainda pode aparecer pela primeira vez um aparelho que o
banco não conhece, com um lacre válido da linha do tempo perdida. Ele pareceria prova de uma
restauração nova e suspenderia todos outra vez. Falta decidir como distinguir: uma ideia é gravar no
lacre a data de emissão e tratar como sobra o que foi emitido antes do fim da última restauração
tratada. É entrega da Fase 1.

## 4. O que cada lado guarda

**No hub**

| Dado | Como |
|---|---|
| Convite | Só o resumo SHA-256, o prazo e o estado. O resumo é apagado quando o convite vence. |
| Identidade do aparelho | Só a parte pública. |
| Credencial lacrada | Só a geração e o resumo da vigente e da pendente, e a maior geração emitida. |
| Login dos admins | Passkeys, que são chaves públicas. Não há senha. |
| Sessão do painel | Só o resumo do identificador. |
| Chave do hub, chave da organização, credencial do motor | Cifradas com a chave-mestra, que fica fora do banco. |
| Arquivo de mensagens | Cifrado com a chave da organização. |
| Fora do banco | A chave-mestra e a marca da auditoria. |

**No Hermes**, na pasta `~/.arauto`, que só o dono lê: para cada aparelho, a identidade e a
credencial mais recente que o conector recebeu, e dados sem segredo. A lista completa está em
[11-conector.md](11-conector.md), seção 2.

## 5. O que o desenho se propõe a garantir

Vale enquanto o hub, a conta do admin e o gerador do sistema estiverem íntegros. São metas do desenho:
só viram garantia depois dos testes da Fase 1 e da Fase 4.

1. Chat ou bloco vazado depois da conexão não contém nada que conecte.
2. Vazado antes do uso, não dá acesso sem um admin autenticado digitar o código do invasor.
3. A parte privada da identidade nasce no Hermes e nunca é transmitida nem mostrada.
4. Nenhum segredo duradouro passa pelo chat, aparece em tela ou entra em registro.
5. Quem leva para fora uma cópia dos arquivos do conector e a usa é descoberto quando o aparelho
   verdadeiro volta a apresentar a credencial dele. Se o Hermes verdadeiro está ligado, com o
   conector em versão que o hub aceita, e alcança o hub, isso acontece em segundos; no pior caso, em
   até 90 segundos. O hub alerta e o aparelho fica parado: em quarentena ou, se quem copiou já o
   tinha encerrado, revogado.
6. Uma cópia deixa de dar acesso depois da primeira troca confirmada que vier depois dela.
7. Revogar e mudar acesso valem na chamada seguinte.
8. Um Hermes só alcança os números que lhe foram dados. Pelo canal, um aparelho não faz nada de
   admin: no máximo pede o próprio encerramento.
9. No canal entre conector e hub, quem termina o TLS no caminho só vê dado cifrado.
10. Quem só lê o banco do hub não consegue se passar por um Hermes.

## 6. O que não é garantido

1. **"A LLM não alcança a credencial."** No modo rápido, o terminal da LLM roda com o mesmo usuário
   do Hermes: lê e grava os arquivos do conector. O que vale: ela não abre o lacre, não precisa do
   segredo, e quem levar uma cópia para fora é descoberto quando o aparelho verdadeiro voltar. Mas um
   processo que continua na máquina e grava na pasta consegue devolver a credencial nova antes de o
   conector voltar, e aí não há sinal. A garantia forte é do modo blindado.
2. **"Ninguém tem acesso."** Quem administra a máquina do Hermes lê os arquivos. O sentido verdadeiro
   é: ninguém precisa ver, digitar ou guardar o segredo.
3. **Uma cópia usada com o Hermes verdadeiro desligado funciona até ele voltar.** Não há limite de
   tempo. Se ele nunca voltar, a cópia nunca é descoberta. O prazo de inatividade, de 30 dias
   ([decisoes/0021](decisoes/0021-prazo-de-inatividade.md)), só pega a cópia usada pela primeira vez
   depois do prazo. Conta como desligado o Hermes cujo
   conector ficou abaixo da versão mínima do hub: ele para de usar o canal, e uma cópia usada com um
   conector atual funciona até o admin reconectar esse Hermes. No reconectar, o uso da cópia pode
   não aparecer como cópia (item 4). Está decidido que o hub passa a conferir a credencial desse
   conector antes de recusar a versão ([decisoes/0030](decisoes/0030-conector-desatualizado.md));
   enquanto a Fase 1 não fixar como, vale este limite.
4. **Se o Hermes verdadeiro volta como aparelho novo, o uso da cópia não aparece como cópia.**
   Acontece quando ele reconecta numa máquina refeita, sem os arquivos antigos. A cópia perde o
   acesso, e o que resta é o admin reconhecer, na aprovação, o último uso do aparelho antigo.
5. **Quem tem a cópia e também segura a rede do Hermes verdadeiro usa o hub enquanto segurar.** Se
   entrar com a conexão do verdadeiro ainda aberta, o painel alerta que ela foi tomada. Se esperar
   essa conexão cair antes de entrar, o painel só mostra uma volta vinda de outro endereço. A
   quarentena só vem quando o verdadeiro falar com o hub.
6. **Quem tem uma cópia antiga, ou só a identidade, consegue tirar o Hermes do ar.** Não ganha acesso,
   mas provoca a quarentena ou o encerramento. Por isso a pasta do conector deve ficar fora das
   cópias de segurança do volume do Hermes.
7. **Depois de o hub voltar de uma cópia antiga**, tudo o que foi revogado, trocado ou retirado
   depois dela se perde. O hub suspende todos os aparelhos ativos e cada Hermes reconecta (C11). Mas uma
   restauração do servidor inteiro por imagem só é percebida quando conecta um aparelho que trocou de
   credencial, ou que foi conectado, depois dela: até lá, uma credencial que já tinha morrido volta a
   valer e um aparelho revogado volta a funcionar. Um admin removido depois da cópia volta a ter
   acesso até ser removido de novo. E, depois de tratada a restauração, um aparelho da linha do tempo
   perdida que o hub nunca viu ainda consegue suspender todos uma vez (C11, Em aberto).
8. Uma LLM manipulada usa as ferramentas que o Hermes já tem, sem roubar nada. Protocolo de conexão
   não resolve isso; só acesso mínimo, limites e auditoria.
9. Uma LLM já manipulada na hora da conexão pode mentir o código ou rodar outra coisa. Contra isso,
   só uma pessoa rodando o comando, ou o modo blindado.
10. A cópia da credencial pode sair pela própria ferramenta de envio, sem nenhum comando de rede que
    a guarda do Hermes veja.
11. Conta de admin tomada ou servidor do hub invadido derrubam tudo.
12. Em toda instalação pelo bloco, o resumo do conector vem do painel. Um hub invadido indica um
    programa falso. Por isso o resumo também fica publicado nas versões do projeto.
13. Quem lê o chat em tempo real consegue atrapalhar a conexão. Não consegue entrar. Quem controla o
    caminho de rede entre o Hermes e o hub impede a conexão enquanto controlar: o desenho protege
    quem entra e o que passa, não a disponibilidade.
14. Engenharia social que leve o admin a digitar o código de outro aparelho.
15. Revogar um Hermes não desliga uma cópia das sessões do motor. Quem as copiou é um dispositivo
    conectado ao número, e só sai pelo celular.
16. O painel depende do TLS de quem instala. O canal cifrado por dentro protege o conector, não o
    navegador do admin.
17. Se a pasta do conector for compartilhada entre dois Hermes, ou o sistema de arquivos não tiver
    trava, o hub trata os dois como cópia e põe o aparelho em quarentena.
18. Um arquivo de credencial perdido ou estragado leva à quarentena, mesmo sem cópia nenhuma.

## 7. A provar antes de implementar

A lista única do que precisa ser provado está em [12-roteiro.md](12-roteiro.md), Fase 1. Dela
dependem, neste documento: o registro do conector no Hermes (C6), a troca da credencial com quedas,
atrasos e um conector que nunca confirma (C7), o reconectar com queda no meio (C10), a restauração
do hub (C11) e o tamanho das mensagens do canal, que no Noise é de 64 KB cada.

Ainda não estão escritos, e são entregas da Fase 1: o formato das mensagens do canal, a conferência
da regra 3, o conteúdo exato do lacre, a função exata do código de conferência e o tratamento das
credenciais que sobram de uma restauração já tratada (C11). Fica também para a Fase 1 o modo como o
hub confere a credencial de um conector abaixo da versão mínima antes de recusá-lo: a regra está
decidida ([decisoes/0030](decisoes/0030-conector-desatualizado.md)), falta a ordem das mensagens
(seção 6, item 3).

## 8. Alternativas consideradas

**TLS 1.3 com desafio assinado, sem Noise.** É mais simples de implementar e basta quando o proxy é
de quem instala. Fica mais fraca em dois casos comuns: rede interna sem TLS e proxy de terceiro que
termina o TLS. Ficou como plano B, registrada em
[decisoes/0012-canal-cifrado-por-dentro.md](decisoes/0012-canal-cifrado-por-dentro.md).

**O fluxo de aparelho que o Hermes já tem** (OAuth, RFC 8628). Não instala nada e não põe segredo no
chat. Mas a credencial ficaria guardada pelo Hermes sem troca com detecção de reuso, não há como
acordar o Hermes por ele, e o hub teria de ser um servidor de autorização. Pode servir, mais adiante,
a agentes que só consultam.
