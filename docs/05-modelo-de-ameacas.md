# 05 — Modelo de ameaças

**Estado: proposta**, acompanhando o desenho de [04-seguranca-conexao.md](04-seguranca-conexao.md).
O que protegemos, de quem, o que cada ataque rende e o que sobra depois das defesas.

## Resumo

```
O QUE O DESENHO TRATA               O QUE SOBRA (as três raízes)
chat ou bloco vazado                conta do admin tomada
credencial copiada                  servidor do hub invadido
banco do hub lido                   motor de WhatsApp exposto
quem termina o TLS do canal
um Hermes tentando alcançar outro
```

**De onde vem este modelo.** Antes de existir código, o desenho passou por várias rodadas de revisão
automática, feitas por agentes de IA que só tinham os documentos. Na primeira, quatro atacaram um
rascunho, cada um de um ponto de partida (o chat vazado, o Hermes manipulado, a rede e o painel, a
cadeia de fornecimento), e um quinto tentou refutar os 51 achados: 34 se sustentaram, inteiros ou em
parte, e nenhum ficou classificado como crítico. Na segunda, quatro leitores sem contexto releram os
documentos já corrigidos e acharam falhas de lógica na troca da credencial, corrigidas na proposta
v0.3. As rodadas seguintes, com um a quatro leitores cada, acharam falhas no hub restaurado de
cópia, no reconectar, no sair e no código de conferência, corrigidas na proposta v0.4. Os relatórios
completos não estão publicados. Revisão de pessoas, de fora, ainda não houve.

## 1. O que protegemos

| Ativo | Por que importa |
|---|---|
| As sessões do motor | Quem as tem **é** o número: lê e envia tudo. |
| O arquivo de mensagens | São conversas de terceiros. |
| A conta do admin | Cria convites, aprova aparelhos e concede acessos. |
| A chave-mestra e a chave do hub | Lacrar credenciais e ser reconhecido como o hub. |
| A identidade e a credencial de cada Hermes | Agir como aquele Hermes. |
| O programa do conector | Roda dentro de cada Hermes. |

## 2. De quem

| Quem | O que consegue de partida |
|---|---|
| Quem lê o canal de comando | O bloco, antes, durante ou depois do uso. |
| Um contato do WhatsApp | Escrever o que o Hermes vai ler. |
| A LLM do Hermes, manipulada | Rodar comandos e ler arquivos com o usuário do Hermes. |
| Alguém na rede | Ver, atrasar e alterar o tráfego entre Hermes e hub, inclusive no proxy que termina o TLS. |
| Alguém com a sessão do painel roubada | Ver o painel. Não tem a passkey. **No hub, em 13/11/2026, a passkey ainda não existe: essa pessoa também conecta e reconecta Hermes, enquanto a sessão valer** ([decisão 0040](decisoes/0040-painel-dentro-do-hub.md)). |
| Quem lê o banco ou a cópia de segurança do hub | Tudo o que estiver gravado ali. |
| Quem invade o servidor do hub | Tudo o que o hub pode. |

## 3. O que cada vazamento dá

| O que vazou | O que o invasor consegue | Por quê |
|---|---|---|
| Chat ou bloco, depois do uso | Nada. | O convite já foi gasto e o código de conferência não é segredo. |
| Chat ou bloco, antes do uso | Atrapalhar a conexão. Acesso, nenhum. | Falta o código que o Hermes verdadeiro disse. |
| Arquivos do conector, copiados com o Hermes ligado | A cópia é barrada em segundos e o aparelho vai para a quarentena. Se quem copiou também segura a rede do Hermes, usa o hub enquanto segurar. | É uma conexão dupla. O painel alerta quando uma conexão aberta é tomada por outra; quem espera a conexão cair antes de entrar só aparece como volta de outro endereço. |
| Arquivos do conector, copiados com o Hermes desligado | Usar o Hermes **até ele voltar**. Na volta, quarentena. Conta como desligado o Hermes com o conector abaixo da versão mínima: a cópia funciona até o admin reconectar, e o uso dela pode não aparecer como cópia. | Para usar, a cópia tem de trocar a credencial, e a do verdadeiro morre. |
| Cópia antiga dos arquivos do conector | Acesso, nenhum, depois da primeira troca que veio depois dela. Mas apresentá-la tira o Hermes do ar. | Reuso leva à quarentena. |
| Tráfego entre conector e hub | Nada. | O canal é cifrado por dentro. |
| Proxy que termina o TLS do canal | Derrubar e atrasar a conexão. | Idem. |
| Proxy que termina o TLS do painel | Ver e alterar o que o admin faz no navegador. | O painel é um site: depende do TLS de quem instala. |
| Domínio ou DNS do hub | Indisponibilidade para os conectores. | O conector só aceita o hub cuja chave veio no bloco. |
| Banco do hub, só lido | Nenhuma credencial de Hermes ou de admin. | Só há chaves públicas e resumos; o resto está cifrado com chave fora do banco. |
| Sessão do painel | Ver o painel. | Toda ação que muda algo pede a passkey na hora. No hub, em 13/11/2026, sem passkey: o que limita é o painel fechado (só por túnel) e o prazo da sessão. |
| **Conta do admin** | **Tudo.** | É uma raiz. |
| **Servidor do hub** | **Tudo daquele hub.** | É uma raiz. |
| **Volume de sessões do motor** | **O próprio WhatsApp.** | É uma raiz. |
| Chave de versões do conector | Publicar um conector falso como atualização. | Fica fora de qualquer hub, guardada à parte. |

## 4. Ameaças e tratamento

### Na conexão

| # | Ameaça | Tratamento | O que sobra |
|---|---|---|---|
| 1 | Quem obteve o convite se inscreve primeiro e é aprovado com um clique. | O admin **digita** o código que o Hermes disse. O que o aparelho declara não decide. Há um só pedido por Hermes, e o código depende de um sorteio do hub: quem se inscreve não escolhe o seu. | Engenharia social que leve o admin a digitar outro código. |
| 2 | O gesto de colar um bloco que roda um programa da internet é imitado por um contato. | O bloco baixa uma versão exata, confere o resumo e só então executa; recusa-se fora do canal de comando; não deixa regra permanente no Hermes. | Um Hermes que obedeça a um bloco falso. A defesa disso é do Hermes: dado não é ordem. |
| 3 | Alguém no meio da rede troca a chave ou o programa na inscrição. | O bloco traz a chave do hub e o resumo do conector. O canal é autenticado desde a primeira mensagem. O código sai do próprio aperto de mãos e de um sorteio do hub. | Um painel falso ou um hub já invadido na hora de copiar o bloco. |
| 4 | Um convite antigo é reapresentado para derrubar um Hermes. | Convite encerrado só gera registro. Não existe bloqueio por segunda tentativa. | Nada conhecido. |
| 5 | O convite aparece em registro de servidor ou em pré-visualização. | O convite não vai em endereço: é argumento do conector e só viaja cifrado, para o hub verdadeiro. | O convite fica no histórico do canal de comando, já gasto. |
| 6 | O hub arquiva a conversa em que o bloco foi colado e a mostra a outro Hermes. | Com o código digitado, rende só atrapalhar. | Atrapalhar a conexão durante 15 minutos. |

### Na credencial

| # | Ameaça | Tratamento | O que sobra |
|---|---|---|---|
| 7 | A LLM, ou outro processo do mesmo usuário, copia os arquivos do conector. A cópia pode sair pela própria ferramenta de envio. | Para usar o hub é preciso trocar a credencial, o que mata a do verdadeiro; um só canal por aparelho; quarentena sem "liberar"; encerramento pedido pelo aparelho gera alerta, e credencial antiga apresentada depois do encerramento também. | A cópia usada com o Hermes desligado funciona até ele voltar, sem limite de tempo. Um processo que continua na máquina e grava na pasta devolve a credencial nova e não deixa sinal. Se ele voltar como aparelho novo, o uso não aparece como cópia: resta o admin reconhecer o último uso na aprovação. |
| 8 | Vários processos do mesmo Hermes parecem cópias e disparam o alarme. | Um líder por Hermes. O hub testa a conexão antiga antes de acusar conexão dupla, e só julga quem responde. | Pasta compartilhada entre dois Hermes, ou sistema de arquivos sem trava: o hub trata como cópia. |
| 9 | A troca da credencial cai no meio e tranca o Hermes do lado de fora. | Troca em duas fases, gravação durável antes do "gravei", primeira credencial pelo mesmo caminho. | O arquivo da credencial perdido ou corrompido: quarentena, reconectar. |
| 10 | Alguém tem uma cópia antiga dos arquivos, ou só a identidade, e quer tirar o Hermes do ar. | Não há defesa: reuso leva à quarentena, e a identidade basta para pedir o encerramento. | Negação de serviço. A pasta do conector deve ficar fora das cópias de segurança. |
| 11 | Alguém substitui o programa do conector no disco do Hermes. | Não há defesa por programa dentro do mesmo usuário. | **Limite do modo rápido.** A defesa é o modo blindado. |
| 12 | O hub volta de uma cópia de segurança antiga. | O hub percebe por uma marca fora do banco, ou por um lacre válido que o banco não explica; suspende todos os aparelhos ativos, encerra convites e pedidos, e avisa; cada Hermes reconecta com convite novo; aparelho desconhecido não apaga nada e aparece numa lista. | O que foi revogado, trocado ou retirado depois da cópia se perde até o admin refazer, inclusive a remoção de um admin. Restauração do servidor inteiro por imagem só é percebida quando conecta um aparelho que trocou de credencial depois dela. |

### No uso

| # | Ameaça | Tratamento | O que sobra |
|---|---|---|---|
| 13 | Uma LLM manipulada usa as ferramentas que o Hermes já tem. | Acesso mínimo por número e por conversa; lista fechada de ferramentas; autorização a cada chamada; tudo na auditoria. | **Ela usa o que tem.** Nenhum protocolo de conexão resolve. |
| 14 | Um Hermes alcança o número de outro. | O Hermes nunca informa sessão do motor; um ponto único chama o motor e só aceita sessão autorizada; identificadores validados. | Erro de implementação. O teste automático disso é critério do MVP ([15](15-mvp.md), M3). |
| 15 | O acesso é retirado e a sessão aberta continua valendo. | Autorização a cada chamada, contra o estado atual. | Nada conhecido. |
| 16 | Um número pareado também é canal de comando de algum Hermes. | O painel permite marcar o número como "de comando" e alerta ao conceder envio nele. | O hub não conhece as listas de cada Hermes. Depende de quem instala. |
| 17 | O hub, invadido, dita o que a LLM lê. | Aviso é ponteiro; a frase fixa e as descrições das ferramentas ficam no conector; o conector não executa comando remoto. | O conteúdo das mensagens, que já é tratado como de terceiros. |

### Na conversa entre Hermes

Entra com o primeiro MVP. O desenho e os limites estão em
[13-conversa-entre-hermes.md](13-conversa-entre-hermes.md).

| Ameaça | Tratamento | O que sobra |
|---|---|---|
| Dois Hermes se acordam um ao outro sem parar. | Tetos fixos por ligação e por destinatário, que abrir um fio novo não zera; teto de acordadas no conector. | Até o teto, eles ainda trocam recados à toa. |
| Um Hermes manipulado escreve a outro. | Só existe recado onde o admin criou a ligação; recado é dado, nunca ordem; a frase fixa diz isso. | **Ele usa as ligações que tem.** O que o outro faz depende das instruções dele. |
| Um Hermes lê ou responde num fio que não é dele. | A leitura e o envio conferem a ligação e a participação a cada chamada; fio de outros responde igual a fio inexistente. | Erro de implementação. Tem teste no MVP. |
| Um remetente enche o banco ou a fila de avisos de outro. | Tetos por dia, de não lidos e de fios novos; um aviso pendente por remetente; expurgo em 30 dias. | Nada conhecido. |
| O hub, invadido, forja um recado. | O nome do remetente tem formato restrito, conferido no conector. | **Raiz.** Não há prova de remetente de ponta a ponta: quem controla o hub escreve como qualquer Hermes. |

### No painel e no hub

| # | Ameaça | Tratamento | O que sobra |
|---|---|---|---|
| 18 | A conta do admin é tomada por phishing, por pedido forjado ou por texto malicioso exibido na tela. | Passkey; confirmação na hora para toda ação que muda algo; todo texto de terceiros escapado; sessão presa ao endereço do painel; primeiro admin só com o código de instalação. | **Raiz.** Quem controla a passkey controla tudo. |
| 19 | Alguém alcança o motor por fora e acrescenta um destino para os eventos. | Motor só na rede interna; o hub confere a configuração de cada sessão do motor e corrige. | **Raiz.** Quem alcança o motor com a chave dele tem o número. |
| 20 | O banco ou a cópia de segurança do hub vaza. | Segredos cifrados com chave fora do banco; admins por passkey; sessões do motor fora da cópia comum; nada de segredo ou conteúdo no registro do sistema, salvo o código de instalação, de uso único. | O arquivo de mensagens, se a chave-mestra vazar junto. |
| 21 | O hub é invadido e indica um conector falso. | Não há atualização empurrada: toda instalação passa pelo bloco, com o resumo à vista; o resumo de cada versão fica publicado fora do hub, com a assinatura da chave de versões. | **Raiz.** Em toda instalação o resumo vem do painel; quem quer certeza compara com o publicado. E o hub invadido lê e envia em todos os números dele. |

### Fora do protocolo

| Ameaça | Tratamento |
|---|---|
| Várias organizações num hub, e uma alcança a outra. | Hoje cada instalação tem uma organização. O banco já separa por organização; com várias, um motor para cada uma. |
| Conector antigo usado para contornar as regras. | Versão mínima exigida; a primeira mensagem do canal nunca muda de formato; uma suíte de cifra só. |
| O hub é levado a buscar um endereço escolhido por outro. | O hub só baixa de endereço público; o endereço do motor só muda por um admin, com passkey. |
| Relógio errado e excesso de pedidos nas portas abertas. | Todo prazo usa só o relógio do hub; convite vencido tem o resumo apagado; limite de pedidos por origem. |
| O WhatsApp bloqueia o número; a licença do motor muda; uma versão nova do Hermes recusa o conector. | O projeto não embute o motor, diz o risco em todo lugar e prevê teste de contrato por versão. |

## 5. O que este modelo não cobre

- A conduta do Hermes: o que ele faz com o que lê.
- A segurança do canal de comando do Hermes, que existe fora do Arauto.
- A máquina de quem administra: quem administra o servidor do Hermes lê os arquivos do conector.

## 6. Como testar

Cada linha da seção 4 que tem regra testável vira ao menos um teste automático antes de a fase
correspondente ser dada como pronta ([12-roteiro.md](12-roteiro.md)). As linhas marcadas como raiz ou
como limite não têm teste: têm aviso. Os testes de regra de segurança são conferidos por mutação:
tira-se a regra do código, e o teste tem de falhar.
