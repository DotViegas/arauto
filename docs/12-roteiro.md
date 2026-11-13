# 12 — Roteiro

**Estado: proposta.** A ordem de construção, com o critério de pronto de cada fase. A ordem pode
mudar; o critério de pronto de cada fase, não.

## Resumo

```
0 PLANEJAMENTO        estes documentos                              feita
1 PROVAS              conferir as suposições e prototipar o canal   no canal: dentro do MVP
2 FUNDAÇÃO DO HUB     banco, acessos, motor sob comando do hub      ┐
4 CONEXÃO             conector, convite, código, credencial lacrada ├ PRIMEIRO MVP, em construção
5 ACORDAR             avisos do hub ao Hermes                       ┘
3 PAINEL              login, números, matriz, auditoria
6 ARQUIVO             histórico guardado, novidades e busca
7 PUBLICAÇÃO          licença, versões assinadas, guia de instalação
```

Em 10/11/2026 as fases 2, 4 e 5 foram reunidas no **primeiro MVP**, que também traz a conversa entre
Hermes ([15-mvp.md](15-mvp.md), [decisão 0033](decisoes/0033-escopo-do-mvp.md)). O painel fica para
depois dele: no MVP, administra-se por comando. O critério de pronto de cada fase continua valendo.

"Publicação" é abrir o repositório ao público. Até lá o projeto não tem versão em uso por terceiros.

## Fase 1 — Provas

Antes de escrever o projeto, tirar as dúvidas que podem mudar o desenho. Esta é a **lista única** do
que precisa ser provado; os outros documentos apontam para cá.

### O ambiente das provas

Ainda não está escrito, e é a primeira entrega da fase: como subir um Hermes Agent nas versões 0.22 e
0.23 para teste, e um motor com um número de teste. Sem isso, quatro das provas abaixo não rodam.

### No Hermes

| O que provar | Como |
|---|---|
| O Hermes aceita o conector como servidor de ferramentas | Registrar um programa mínimo pelo comando oficial, nas versões 0.22 e 0.23, e reiniciar o Hermes. Medir também em quanto tempo ele inicia o programa sem ser reiniciado. |
| Como o conector obtém a chave da API local | Testar o primeiro caminho de [11-conector.md](11-conector.md), seção 6; se ele não funcionar, o segundo ([decisão 0023](decisoes/0023-chave-da-api-local.md)). |
| O bloco chega idêntico pelo WhatsApp | Colar pelo celular e pelo computador e comparar byte a byte do lado do Hermes. |
| O que o Hermes faz ao receber o bloco | Se pede aprovação humana, se guarda o procedimento em memória, quanto tempo o terminal dá ao comando. |
| Quantas cópias do conector o Hermes inicia | Subir todos os processos do Hermes e contar. |
| O Hermes reinicia um servidor de ferramentas que encerra | Encerrar o programa mínimo e ver se, e em quanto tempo, ele volta. |
| A chamada à API local | Endereço, corpo, em que conversa a execução acontece e como usar o identificador de repetição. |
| O que muda na versão 0.23 | Repetir cada linha de [hermes-fatos](../referencia/hermes-fatos.md). |

### No canal

Desde 10/11/2026 estas provas são feitas dentro do MVP, como testes da suíte
([15-mvp.md](15-mvp.md), seção 7). Em 11/11/2026 o lado do hub já tem teste, contra um conector de
teste e só em Windows, para: o aperto de mãos e a inscrição; a troca com quedas sorteadas dos dois
lados; a troca de 24 horas com o "gravei" que não chega; a cópia com o verdadeiro ligado e desligado;
o hub restaurado com o hub no ar e com o `admin` antes do `serve`; e as mensagens cortadas em
pedaços. Nenhuma está "confirmada" ainda: falta o conector do projeto, e as provas que dependem dele
(arquivo perdido, troca de pasta, comandos repetidos, sair e reconectar cruzados).

| O que provar | Como |
|---|---|
| O aperto de mãos e a inscrição | Dois programas mínimos, com o convite e o código de conferência. |
| O código não pode ser escolhido | Do lado de quem se inscreve, tentar chegar a um código alvo refazendo a inscrição. |
| A troca da credencial aguenta queda | Quedas sorteadas nos dois lados, em qualquer ponto da troca, inclusive na primeira. |
| A troca aguenta atraso | Mensagens entregues atrasadas e fora de ordem. |
| A ordem de gravação do hub | O hub cai entre gravar a pendente e enviar a nova, e entre ouvir o "gravei" e gravar a promoção. |
| A troca de 24 horas | A troca dentro da sessão aberta, com queda no meio e com um "gravei" que não chega. |
| A cópia aparece | Um conector que apresenta a credencial e nunca confirma; uma cópia que conecta com o verdadeiro desligado e com ele ligado. |
| O arquivo perdido | O conector sem o arquivo da credencial, e com o arquivo antigo depois de uma queda de energia simulada. |
| O hub restaurado | O banco voltando de uma cópia antiga: com aparelhos que trocaram de credencial depois dela, com um aparelho reconectado depois dela, com a cópia tirada sem o hub reiniciar, e com o servidor inteiro voltando de uma imagem. A queda entre gravar o banco e gravar a marca. |
| Reconectar com queda | A máquina cai no meio da troca de pasta do aparelho. |
| Comandos repetidos | `join` e `leave` rodados de novo em cada situação: pendente, aprovado, vencido, revogado, hub fora do ar. |
| Sair e reconectar cruzados | `leave` no meio de um reconectar; convite de outra linha colado num Hermes já conectado; `leave` feito por quem tem uma cópia. |
| Mensagens grandes | Como dividir o que passa de 64 KB. |

Entregas escritas da fase: o formato das mensagens do canal, o conteúdo do lacre, a função do código
de conferência, a conferência da regra 3 da troca ([04](04-seguranca-conexao.md), C7), o tratamento
das credenciais que sobram de uma restauração já tratada (C11) e o formato do soquete local do
conector. Define-se também como o hub confere a credencial de um conector abaixo da versão mínima
antes de recusá-lo: a regra está decidida ([decisão 0030](decisoes/0030-conector-desatualizado.md)),
falta a ordem das mensagens ([04](04-seguranca-conexao.md), seção 6, item 3). Tudo isso está
escrito, como proposta, em [14-protocolo-do-canal.md](14-protocolo-do-canal.md).

**Pronto quando:** cada suposição está marcada "confirmada" ou "não vale, e o desenho mudou"; e o
protótipo passa por milhares de rodadas sorteadas sem trancar do lado de fora um aparelho legítimo
que manteve os arquivos, sem aceitar uma credencial reusada e sem deixar uma cópia levada para fora
usar o hub sem que isso apareça quando o verdadeiro voltar a apresentar a credencial dele.

As decisões que travavam o código foram tomadas em 10/11/2026: nomes no código em inglês, teto de
envio fixo, inatividade de 30 dias, a ordem de tentativa para a chave da API local e envio de arquivo
por endereço público ([decisões](decisoes/README.md)). O nome novo do projeto foi aplicado
aos documentos em 10/11/2026 ([decisão 0026](decisoes/0026-nome-do-projeto.md)).

## Fase 2 — Fundação do hub

O código do hub é escrito do zero ([decisão 0033](decisoes/0033-escopo-do-mvp.md)). O serviço de
origem é particular e não é copiado; as ideias que vêm dele já estão nestes documentos. Os nomes no
código entram em inglês ([decisão 0016](decisoes/0016-idioma.md)).

Entram: o banco com organização, Hermes, número e acesso; o adaptador do motor como único ponto de
contato; o hub como dono da configuração das sessões do motor; as ferramentas com o argumento
`number`; a autorização a cada chamada; o teto de envio, com o valor fixado nesta fase; a recusa de
endereço que leve à rede interna, com teste; e os programas falsos (motor e Hermes) para os testes.

**Pronto quando:** existe teste automático de que um Hermes não alcança o número de outro; retirar um
acesso com a sessão aberta vale na chamada seguinte; os testes rodam contra um motor falso; o hub
detecta e corrige uma sessão do motor configurada por fora.

## Fase 3 — Painel

Primeiro acesso, login por passkey, as telas de Hermes, WhatsApps, parear, acessos e auditoria.

**Pronto quando:** um número de teste é pareado por código e por QR pela tela; nenhuma tela, registro
ou auditoria mostra convite, QR ou credencial fora do momento de uso; um nome de contato com código
malicioso aparece como texto; uma ação que muda algo, sem a passkey, é recusada.

## Fase 4 — Conexão

O conector com os quatro comandos, o líder único, o convite, o código digitado, a credencial lacrada,
a quarentena, revogar, reconectar e sair. As telas de conectar e de confirmar. O prazo de
inatividade, com o rótulo da tela que ainda falta
([decisão 0021](decisoes/0021-prazo-de-inatividade.md)).

**Pronto quando:**

- Do zero ao primeiro Hermes lendo um número leva menos de 15 minutos, seguindo só o que a tela diz.
- Cada falha do catálogo tem mensagem própria nos dois lados.
- Copiar a pasta do conector para outra máquina e usar leva à quarentena quando o verdadeiro conecta.
- Revogar derruba o canal em menos de 5 segundos.
- Existe um teste para cada linha do [modelo de ameaças](05-modelo-de-ameacas.md) que tem regra
  testável.

## Fase 5 — Acordar

A fila de avisos, a frase fixa e a chamada à API local do Hermes.

**Pronto quando:** com quedas sorteadas do hub, do conector e do Hermes, nenhum aviso some antes de
expirar e nenhum aviso repetido inicia uma segunda execução no Hermes; o aviso não carrega texto
escrito por terceiros.

## Fase 6 — Arquivo

Importar o histórico que o motor guarda, gravar a mensagem inteira e os arquivos na chegada, e as
ferramentas de novidades e de busca.

**Pronto quando:** o Hermes lê uma conversa com o motor desligado; a busca acha por texto, por pessoa
e por período; o expurgo apaga o que venceu, inclusive os arquivos.

Até esta fase, toda leitura depende do motor estar no ar.

## Fase 7 — Publicação

Licença no repositório (Apache-2.0), com o titular dos direitos definido, registro de marca do nome
conferido, histórico do repositório
limpo, README em inglês com as traduções ([decisão 0016](decisoes/0016-idioma.md)), versões
assinadas com o resumo publicado, guia de instalação (com o painel fechado por padrão), relato
privado de falha ligado no GitHub e a página de quem mantém.

**Pronto quando:** alguém de fora instala o hub, conecta um Hermes e pareia um número seguindo só o
guia.

## Depois

| Item | Observação |
|---|---|
| Modo blindado | O conector separado do Hermes, instalado por uma pessoa. |
| Segundo motor | Adaptador para a API oficial do WhatsApp. |
| Conversa entre Hermes: salas e observador | O recado entre dois Hermes já entra no primeiro MVP ([13](13-conversa-entre-hermes.md)). |
| Outros meios de conversa | E-mail, Telegram e outros, na mesma tela de conexão. |
| Gestão da frota | Criar, atualizar e monitorar os Hermes pelo painel. Exige rever a regra de que o conector não executa nada por ordem do hub. |
| Várias organizações num hub | Com um motor por organização. |

## Como se verifica

O padrão vem do serviço de origem e vale para todas as fases:

- **Motor falso e Hermes falso** nos testes automáticos, para rodar tudo sem WhatsApp de verdade.
- **Rodadas sorteadas** nas filas e na troca de credencial: o teste derruba, atrasa e repete em
  pontos aleatórios e confere que nada se perde nem se repete.
- **Mutantes** nas regras de segurança: altera-se o código de propósito, e algum teste tem de falhar.
- **Canário:** um segredo conhecido é plantado nos testes e não pode aparecer em nenhuma saída,
  registro ou resposta.
- **Revisão adversarial** antes de cada publicação.
- **Teste de contrato**, feito à mão, contra o motor e o Hermes de verdade, com um número de teste, a
  cada versão nova de um deles. É a única exceção à regra de não testar em número real.
