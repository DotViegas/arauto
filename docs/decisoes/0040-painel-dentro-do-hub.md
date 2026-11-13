# 0040 — O painel dentro do hub: porta própria, entrada por código e as primeiras telas

**Estado:** proposta · **Data:** 12/11/2026, com o código da primeira entrega do painel no hub
(telas de leitura); 13/11/2026, com a segunda (conectar Hermes)

## Contexto

A [0033](0033-escopo-do-mvp.md) deixou o painel para depois do MVP: administra-se por comando, no
servidor. A [0019](0019-painel-com-passkey.md) diz como ele deve ser quando existir: login por
passkey, e, com a sessão do painel sozinha, só se vê. Em 12/11/2026 quem mantém o projeto pediu o
painel e aprovou o desenho das telas.

A passkey ainda não foi construída. Esta decisão registra o que as duas primeiras entregas do
painel fazem, o que o código escolheu onde os textos são omissos e onde ele se afasta deles. Tudo
foi testado só pelos testes automáticos, contra um hub de teste: o painel nunca rodou num servidor,
num container nem atrás de um proxy, e nunca conectou um Hermes de verdade.

## Decidido por quem mantém, em 12/11/2026

- **A entrada é por código enquanto a passkey não existir.**
- **A sessão do painel aberta por esse código pode agir** (conectar Hermes, parear, dar acesso).
  Isso muda a 0019, que pedia a passkey em cada ação, e está anotado nela.
- **Nesse período o painel fica fechado:** a porta dele só é alcançada na própria máquina do hub,
  por túnel. A passkey volta antes de o painel ser aberto à internet.

## Decisão

### Onde o painel mora

- **Dentro do hub, numa terceira porta.** Não é outro programa: usa as mesmas funções que o
  `arauto-hub admin` usa, com a mesma conferência da organização.
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
- **Um script só, que é um arquivo do painel.** Nenhuma página leva script escrito dentro dela, e
  a política de conteúdo enviada em toda resposta (`default-src 'none'`, script, folha e fontes só
  do próprio painel, `form-action 'self'`, `frame-ancestors 'none'`) impede o navegador de rodar
  outro. Só as páginas do guia carregam o arquivo. Ele ajuda a tela (copiar o bloco, a animação, a
  contagem do prazo): não escreve marcação, não lê cookie e não chama endereço nenhum. Sem ele,
  cada passo continua funcionando, porque cada passo é uma página do hub.
- **A folha de estilo, as fontes, o script e o mascote vão dentro do programa.** As fontes são
  livres, com a licença ao lado.

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

### As telas de leitura

- **Hermes, WhatsApps e Acessos, com a faixa de alertas no topo.** Mostram o que
  `arauto-hub admin` mostra, mais o que só o hub em execução sabe: quais Hermes estão conectados
  agora.
- **O que as telas nunca mostram:** código de entrada, código de conferência, endereço ou chave do
  motor, nome da sessão do motor e conteúdo de mensagem.

### Conectar Hermes

O guia do [02-como-funciona.md](../02-como-funciona.md), seção 1, numa janela sobre a lista de
Hermes, com o percurso das etapas no topo: nome, bloco, código, pronto.

- **A regra de cada ação continua num lugar só.** Criar a linha, emitir o convite, aprovar e negar
  são as funções que `arauto-hub admin` chama, com as mesmas conferências, a mesma auditoria e a
  mesma marca. Do painel é só a ordem das telas.
- **O convite aparece uma vez,** na resposta do formulário que o emitiu. Nenhum endereço aberto
  por leitura emite, mostra ou gasta um convite, e o banco continua guardando só o resumo.
- **O texto do bloco é o proposto no 02,** palavra por palavra; o painel só preenche os campos.
  Onde o 02 escreve `arauto-connector-{VERSAO}`, vai o nome do arquivo que o hub serve, com o
  SHA-256 calculado dos bytes servidos.
- **Sem conector para servir, o painel não faz bloco** e não emite nada: manda usar o comando.
  Com mais de um conector para Linux na pasta, o bloco usa o primeiro pelo nome.
- **O código de conferência é digitado e nunca mostrado,** nem de volta a quem o errou.
- **Cada etapa acende quando o hub a observa:** convite emitido, aparelho usou o convite, pedido
  aprovado, conector confirmou a primeira credencial. A página que espera pergunta de novo sozinha,
  e essa pergunta só lê.
- **No reconectar, a resposta sobre o último uso vale para o uso que estava na tela.** Se o
  aparelho atual foi usado enquanto o admin respondia, nada é aprovado e o uso novo é mostrado.
- **Fechar o guia não cancela o convite.** Ele vence sozinho, em 15 minutos, ou é cancelado por
  outro convite; a linha do Hermes leva de volta ao guia.

## O que não é garantido

- O código de entrada não identifica uma pessoa. Quem roda o comando já tem o banco e a
  chave-mestra; toda sessão do painel é do "admin local".
- **Uma sessão do painel roubada age enquanto valer** (12 horas, ou 30 minutos sem uso). Com ela
  dá para ver tudo o que as telas mostram e para reconectar um Hermes num aparelho de quem a
  roubou, que então herda os acessos daquele Hermes. É o risco aceito em 12/11/2026, e a razão de
  o painel ficar fechado até a passkey existir.
- O painel depende do TLS de quem instala. Sem TLS ele só é aceito em `localhost`, para uso por
  túnel, e aí o cookie não leva `Secure`.
- Num container, a porta do painel escuta em todas as interfaces: quem está na mesma rede do hub,
  como o motor de WhatsApp, alcança a tela de entrada. O que segura ali é o código de uso único.
- O bloco passa pela tela e pela área de transferência de quem o copia. O convite dentro dele vale
  15 minutos e um uso.

## Decisões pendentes, de quem mantém

1. **O idioma do texto das telas.** A [0016](0016-idioma.md) não trata dele. As telas estão em
   português, como o protótipo aprovado.
2. **O nome "código de entrada".** Entrou no glossário como proposta. A alternativa é alargar o
   "código de instalação".
3. **O texto do bloco.** O painel usa o proposto no 02, em português. O comando imprime outro, em
   inglês, com as mesmas linhas de comando. Falta escolher um, ou dizer que são dois de propósito.
4. **O [04-seguranca-conexao.md](../04-seguranca-conexao.md)** diz, em duas tabelas, que o login do
   painel é por passkey. Ele não foi alterado: muda só com aprovação.

## Consequências

- A 0033 ganha a nota de que o painel começou antes do fim do MVP, e a 0019 a de que a sessão do
  painel age enquanto a passkey não existir. A [0028](0028-painel-fechado-por-padrao.md) não muda.
- O hub passa a ter um endereço de administração na rede, quando quem instala o liga. O guia de
  instalação publica a porta do painel só em `127.0.0.1`.
- Um banco que já tem dados precisa da migração nova antes de o `serve` subir.
- Alternativas consideradas: um programa separado para o painel (outra credencial de banco e outra
  superfície para proteger); uma biblioteca de modelos (escape automático, mas uma dependência a
  mais); guardar o último uso da sessão no banco (sobrevive ao reinício do hub, mas é uma gravação
  em requisição de leitura); senha (a 0019 já a descartou); um painel sem script nenhum (o bloco
  não seria copiado com um clique, que é o que o desenho aprovado pede).
