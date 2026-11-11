# Segurança

> **Estado: primeiro MVP em construção.** Ainda não há versão para uso. Este arquivo descreve o que o desenho se propõe a
> garantir, o que ele não garante e como relatar uma falha.

## Como relatar uma falha

**Não abra uma issue pública para falha de segurança.**

O canal será o relato privado de falha do GitHub, na aba de segurança do repositório
([decisão 0027](docs/decisoes/0027-canal-de-relato-de-falha.md)). Ele passa a existir quando o
repositório for aberto ao público. Até lá, o projeto não tem versão em uso por terceiros.

Ao relatar, inclua: o que você conseguiu, os passos para repetir, a versão do hub e do conector e o
que um invasor ganharia.

## O que o desenho se propõe a garantir

São metas. Só viram garantia depois dos testes previstos no [roteiro](docs/12-roteiro.md). Valem
enquanto o hub, a conta do admin e o gerador aleatório do sistema estiverem íntegros.

1. A conversa em que você colou o bloco de conexão, se vazar **depois** do uso, não contém nada que
   conecte.
2. Se vazar **antes** do uso, quem a leu não consegue acesso sem um admin autenticado digitar o
   código dele.
3. Nenhum segredo duradouro passa pelo chat, aparece em tela ou entra em registro.
4. Quem leva para fora uma cópia dos arquivos de um Hermes e a usa é descoberto quando o aparelho
   verdadeiro volta a apresentar a credencial dele: em segundos, se ele está ligado, em versão que o
   hub aceita, e alcança o hub (no pior caso, 90 segundos); na volta, se estava desligado ou com a
   rede segurada por quem copiou. O hub alerta e o aparelho fica parado.
5. Um Hermes só alcança os números que lhe foram dados.
6. Revogar um Hermes ou tirar um acesso vale na chamada seguinte.
7. Quem só lê o banco do hub não consegue se passar por um Hermes.

O detalhe está em [docs/04-seguranca-conexao.md](docs/04-seguranca-conexao.md).

## O que o projeto não garante

Dizemos isto antes para ninguém descobrir depois.

1. **A LLM do Hermes alcança os arquivos do conector** quando é o próprio Hermes que o instala. Ela
   não consegue abrir a credencial, mas o arquivo está ao alcance dela, para ler e para gravar. Um
   processo que continue na máquina, com o mesmo usuário, consegue usar uma cópia fora sem deixar
   sinal. Quem precisa que ela não alcance usa o modo blindado, que ainda não existe.
2. **Uma cópia da credencial, usada com o Hermes verdadeiro desligado, funciona até ele voltar.** Não
   há limite de tempo. Se ele nunca voltar, a cópia nunca é descoberta. O mesmo vale enquanto quem
   copiou segurar a rede do Hermes verdadeiro. E um Hermes com o conector abaixo da versão mínima
   do hub conta como desligado: a cópia funciona até o admin reconectá-lo, e o uso dela pode não
   aparecer como cópia. Está decidido que o hub vai conferir a credencial desse conector antes de
   recusá-lo ([decisão 0030](docs/decisoes/0030-conector-desatualizado.md)); enquanto isso não
   estiver provado, vale este limite.
3. **Quem tem uma cópia antiga dos arquivos do conector, ou só a identidade, consegue tirar o Hermes
   do ar.** Não ganha acesso, mas provoca a quarentena ou o encerramento.
4. **Uma LLM manipulada usa as ferramentas que o Hermes já tem.** Se um Hermes pode enviar num
   número, uma instrução maliciosa que ele obedeça consegue enviar por ele. O projeto limita o
   alcance e registra tudo; não impede.
5. **Conta de admin tomada ou servidor do hub invadido derrubam tudo.**
6. **Quem tem o volume de sessões do motor tem o WhatsApp.** O motor precisa ficar sem porta pública.
7. **Restaurar o hub de uma cópia antiga desfaz** o que foi revogado, trocado ou retirado depois
   dela. O hub suspende os aparelhos ativos e avisa; cada Hermes precisa ser reconectado. Se o
   servidor inteiro voltar de uma imagem, o hub só percebe quando conectar um aparelho que trocou de
   credencial, ou foi conectado, depois dela; até lá, o que foi revogado volta a valer. Em qualquer
   restauração, um admin removido depois da cópia volta a ter acesso até ser removido de novo.
8. **Em toda instalação pelo bloco, o resumo do conector vem do painel.** Um hub invadido indica
   um programa falso. Confira com o resumo publicado nas versões do projeto.
9. **O WhatsApp pode bloquear o número.** O motor é um cliente não oficial.

O projeto nunca vai dizer que "fecha todas as brechas".

## Raízes de confiança

Três coisas, se comprometidas, dão acesso a tudo de uma instalação. Proteja-as nesta ordem:

| Raiz | Como proteger |
|---|---|
| A conta do admin | Passkey. O painel vem fechado, em rede privada, por padrão; sempre com TLS. |
| O servidor do hub | Atualizações em dia, acesso restrito, chave-mestra fora do banco e fora da cópia comum. |
| O motor de WhatsApp | Só na rede interna, com chave própria da instalação. |

## Versões do conector

O desenho prevê que toda versão publicada do conector seja assinada com a chave de versões do
projeto, que não fica em hub nenhum, e que o resumo SHA-256 de cada versão fique publicado no
repositório. O bloco de conexão traz esse resumo: confira com o publicado.
