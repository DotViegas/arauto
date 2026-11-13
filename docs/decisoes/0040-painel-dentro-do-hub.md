# 0040 — O painel dentro do hub: porta própria, entrada por código e telas de leitura

**Estado:** proposta · **Data:** 12/11/2026, com o código da primeira entrega do painel no hub

## Contexto

A [0033](0033-escopo-do-mvp.md) deixou o painel para depois do MVP: administra-se por comando, no
servidor. A [0019](0019-painel-com-passkey.md) diz como ele deve ser quando existir: login por
passkey, e, com a sessão do painel sozinha, só se vê. Em 12/11/2026 quem mantém o projeto pediu o
painel e aprovou o desenho das telas.

A passkey ainda não foi construída. Esta decisão registra o que a primeira entrega do painel faz,
o que o código escolheu onde os textos são omissos e onde ele se afasta deles. Tudo foi testado só
pelos testes automáticos, contra um hub de teste: o painel nunca rodou num servidor, num container
nem atrás de um proxy.

## Decisão

### Onde o painel mora

- **Dentro do hub, numa terceira porta.** Não é outro programa: usa as mesmas funções do
  repositório que o `arauto-hub admin` usa, com a mesma conferência da organização.
- **Desligado por padrão.** O painel só existe com as duas variáveis: `ARAUTO_LISTEN_PANEL` (onde
  a porta escuta) e `ARAUTO_PANEL_URL` (o endereço que o admin abre no navegador, com as mesmas
  regras do `ARAUTO_PUBLIC_URL`). Uma sem a outra impede o `serve` de subir, e o endereço da porta
  não pode ser o de nenhuma das outras duas.
- **Cada porta só responde o que é dela.** Nenhum endereço do painel existe na porta dos
  conectores nem na interna, e nenhum endereço delas existe na do painel
  ([03-arquitetura.md](../03-arquitetura.md), seção 4: o login do admin nunca viaja para a porta
  dos conectores).
- **A requisição tem de nomear o painel.** O cabeçalho `Host` é o do `ARAUTO_PANEL_URL`; qualquer
  outro recebe 421 antes de qualquer leitura.
- **Páginas escritas pelo próprio programa, sem biblioteca de modelos.** A marcação é texto fixo
  do programa; todo valor passa por uma única função de escape. O 03 previa "modelos de escape
  automático": o escape continua obrigatório, só não vem de uma biblioteca.
- **Nenhum script.** As páginas não têm JavaScript, e a política de conteúdo enviada em toda
  resposta (`default-src 'none'`, folha e fontes só do próprio painel, `form-action 'self'`,
  `frame-ancestors 'none'`) impede o navegador de rodar um. A folha de estilo e as fontes vão
  dentro do programa; as fontes são livres, com a licença ao lado.

### A entrada

- **Código de entrada.** O comando `arauto-hub admin panel code`, rodado na máquina do hub, sorteia
  20 bytes no gerador do sistema e mostra o código uma vez. O hub guarda só o SHA-256 dele. Vale
  10 minutos e uma entrada; emitir outro cancela o anterior.
- **O código é digitado, nunca mostrado pelo painel,** e só é gasto por um formulário enviado
  (`POST`). Ele não vai em endereço: abrir uma página nunca gasta um código.
- **Uma frase só para toda recusa** (código errado, vencido, já usado), e um teto de 30 consultas
  de código por minuto, de todos juntos. O teto não protege o código, que tem 160 bits: afasta uma
  enxurrada do banco.
- **Peças usadas:** SHA-256 com um prefixo fixo para cada uso, o gerador do sistema, e um cookie
  `HttpOnly` e `SameSite=Strict` (com `Secure` e o prefixo `__Host-` quando o endereço do painel é
  `https`). Nenhuma cifra própria.

### A sessão do painel

- **O identificador** tem 32 bytes do gerador do sistema e vive só no cookie. O hub guarda o
  SHA-256 dele, e apaga esse resumo quando fecha a sessão do painel.
- **Dura 12 horas, ou 30 minutos sem uso,** como o [07-telas.md](../07-telas.md) previa.
- **O tempo sem uso fica na memória do hub, não no banco,** para que uma requisição de leitura não
  grave nada. Consequência: quando o hub reinicia, toda sessão do painel acaba, e o admin entra de
  novo com outro código.
- **Cada requisição confere a sessão do painel no banco** (resumo, estado e validade), sem memória
  intermediária do que foi autorizado antes.
- **Um formulário só é aceito com o `Origin` do painel.** Sem ele, ou com outro, a resposta é 403
  antes de o formulário ser lido.
- **Na auditoria:** a entrada é evento de segurança (`panel.signed_in`); o código emitido
  (`panel.code_issued`) e a saída (`panel.signed_out`) são eventos comuns. O ator é o admin local;
  onde o comando grava o usuário do sistema, o painel grava a palavra fixa `panel`.
- **Expurgo:** as linhas dos códigos e das sessões do painel saem 30 dias depois do momento em que
  deixaram de valer. O que cada uma fez continua na auditoria.

### O que as telas fazem

- **Três telas, só de leitura:** Hermes, WhatsApps e Acessos, com a faixa de alertas no topo.
  Mostram o que `arauto-hub admin` mostra, mais o que só o hub em execução sabe: quais Hermes estão
  conectados agora.
- **Nada muda pelo painel nesta entrega.** Conectar, parear e dar acesso continuam pelo comando.
  Isso mantém a regra da 0019: com a sessão do painel, sozinha, só se vê.
- **O que as telas nunca mostram:** código de entrada, código de conferência, endereço ou chave do
  motor, nome da sessão do motor e conteúdo de mensagem.

## O que não é garantido

- O código de entrada não identifica uma pessoa. Quem roda o comando já tem o banco e a
  chave-mestra; toda sessão do painel é do "admin local".
- Uma sessão do painel roubada vê tudo o que as telas mostram: nomes dos Hermes, telefones dos
  números, acessos e alertas.
- O painel depende do TLS de quem instala. Sem TLS ele só é aceito em `localhost`, para uso por
  túnel, e aí o cookie não leva `Secure`.
- Num container, a porta do painel escuta em todas as interfaces: quem está na mesma rede do hub,
  como o motor de WhatsApp, alcança a tela de entrada. O que segura ali é o código de uso único.

## Decisões pendentes, de quem mantém

1. **Entrada por código ou passkey.** A 0019 decide a passkey. A proposta aqui é o código de
   entrada valer enquanto a passkey não existir. Falta confirmar, e dizer o que acontece com ele
   depois.
2. **Ações que mudam algo.** A 0019 pede a passkey em cada uma. As próximas entregas do painel
   (conectar Hermes, parear, dar acesso) esbarram nisso: ou a passkey vem antes da primeira ação,
   ou a 0019 muda. Esta entrega não decide: não muda nada.
3. **O idioma do texto das telas.** A [0016](0016-idioma.md) não trata dele. As telas estão em
   português, como o protótipo aprovado.
4. **O nome "código de entrada".** Entrou no glossário como proposta. A alternativa é alargar o
   "código de instalação".
5. **O [04-seguranca-conexao.md](../04-seguranca-conexao.md)** diz, em duas tabelas, que o login do
   painel é por passkey. Ele não foi alterado: muda só com aprovação.

## Consequências

- A 0033 ganha a nota de que o painel começou antes do fim do MVP. A 0019 e a
  [0028](0028-painel-fechado-por-padrao.md) não mudam.
- O hub passa a ter um endereço de administração na rede, quando quem instala o liga. O guia de
  instalação publica a porta do painel só em `127.0.0.1`.
- Um banco que já tem dados precisa da migração nova antes de o `serve` subir.
- Alternativas consideradas: um programa separado para o painel (outra credencial de banco e outra
  superfície para proteger); uma biblioteca de modelos (escape automático, mas uma dependência a
  mais); guardar o último uso da sessão no banco (sobrevive ao reinício do hub, mas é uma gravação
  em requisição de leitura); senha (a 0019 já a descartou).
