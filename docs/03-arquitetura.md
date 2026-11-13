# 03 — Arquitetura

**Estado: proposta.** As peças, o que cada uma faz e por onde elas se falam. Ainda não há código: os
verbos no presente descrevem o desenho.

## Resumo

```
      pessoas                                   WhatsApp
         │                                          │
      PAINEL                                      MOTOR   (só rede interna)
         │                                          │
 ┌───────┴──────────────── HUB ─────────────────────┴───────┐
 │  painel │ porta dos conectores │ caixa preta │ adaptador  │
 │  núcleo: acessos, filas, arquivo de mensagens, auditoria  │
 └──────────────────────────▲────────────────────────────────┘
                            │  canal de saída: websocket, cifrado por dentro
                            │
                        CONECTOR   (ao lado do Hermes)
                            ▲
                            │  ferramentas (MCP por stdio)
                         HERMES ◄──── canal de comando (o operador;
                                      fora do Arauto)
```

O projeto tem dois programas, o **hub** e o **conector**, e usa um terceiro, o **motor**, que é de
quem instala.

## 1. O que cada peça faz

| Peça | Faz | Não faz |
|---|---|---|
| **Hub** | Mostra o painel, autentica conectores, guarda as conversas, decide cada acesso, comanda o motor. | Não responde mensagem, não filtra conteúdo, não escolhe qual Hermes age. |
| **Conector** | Entrega as ferramentas ao Hermes, mantém o canal com o hub, acorda o Hermes. | Não decide acesso, não fala com o motor, não executa comando do hub. |
| **Motor** | Fala com o WhatsApp. | Não é alcançado por ninguém além do hub. |

## 2. Fronteiras

- **Só o hub fala com o motor.** O Hermes nunca recebe endereço nem credencial do motor. Arquivos e
  mensagens passam pelo canal.
- **O Hermes só abre conexão de saída.** Nenhuma porta de rede nova escuta na máquina do Hermes; o
  conector usa só um soquete local, entre as próprias cópias. Um Hermes atrás de NAT conecta do mesmo
  jeito.
- **Hub e motor são processos separados.** O hub manda no motor pela API dele. Uma versão nova do hub
  não derruba as sessões do motor.
- **O hub é a fonte da verdade sobre o motor.** Ele cria as sessões do motor, configura o destino dos
  eventos e confere tudo de novo a cada partida e a cada 5 minutos. Se achar um destino de eventos
  que não é o hub, corrige e alerta.

## 3. Módulos do hub

| Módulo | Responsabilidade |
|---|---|
| `panel` | Telas e ações do admin. No hub, em 12/11/2026: só as telas de leitura ([decisão 0040](decisoes/0040-painel-dentro-do-hub.md)). |
| `enrollment` | Recebe pedidos de conexão e serve o download do conector. |
| `channel` | O websocket com cada conector: aperto de mãos, troca da credencial, chamadas e avisos. |
| `vault` | A caixa preta: sorteia, lacra e abre credenciais. Único módulo que toca a chave-mestra. |
| `core` | Organizações, Hermes, acessos, filas e auditoria. Sem regra de negócio. |
| `archive` | O arquivo de mensagens e a busca nele. |
| `peers` | A conversa entre Hermes: lista de pares, fios, recados e tetos ([13](13-conversa-entre-hermes.md)). |
| `engine` | O único cliente do motor e o único receptor dos eventos dele. |

O módulo `engine` fica atrás de uma interface, `WhatsappEngine`, com quatro grupos de operação: a vida
da sessão do motor (criar, configurar, ligar, reiniciar, parear, sair), as conversas (listar, ler,
grupos, contatos), o envio (texto, arquivo, reação, editar, apagar) e os eventos. O WAHA é um
adaptador dela. Um segundo adaptador, para a API oficial do WhatsApp, cabe no mesmo lugar. A lista
exata de operações e o motor falso dos testes são entregas da Fase 2.

O desenho prevê um único ponto do código que chama o motor, e ele só aceita uma sessão do motor já
autorizada. O tipo que representa "autorizada" só pode ser criado pela função que confere o acesso.
A ideia é o isolamento sair da construção, e não de conferências espalhadas.

## 4. Portas

| Escuta | Quem usa | Onde pode ficar |
|---|---|---|
| **Painel** | Admins, pelo navegador | Por padrão, só rede privada ou VPN. Quem instala pode abri-lo à internet ([decisoes/0028](decisoes/0028-painel-fechado-por-padrao.md)). |
| **Conectores** | Os conectores: download, inscrição e canal | Precisa ser alcançável pelos Hermes. |
| **Eventos do motor** | O motor | Só rede interna. |
| **Saúde e métricas** | Monitoramento | Só rede interna. |

Painel e conectores ficam em endereços diferentes. Assim o login do admin nunca viaja para a porta
dos conectores. O TLS das duas portas é de quem instala. No hub, em 12/11/2026, o painel tem uma
porta própria, desligada por padrão, e nenhum endereço dele existe nas outras duas
([decisão 0040](decisoes/0040-painel-dentro-do-hub.md)).

## 5. O que roda onde

Hoje o desenho é de uma instalação por organização, no servidor de quem instala: hub, banco, motor e
chaves ficam ali, e o conector fica ao lado de cada Hermes.

O banco já nasce separado por organização. Isso deixa aberta a possibilidade de um hub servir a
várias organizações, com um motor para cada uma, sem reescrita. O que **não** se constrói agora:
cadastro aberto ao público e limites de uso por organização.

## 6. Tecnologia

| Parte | Escolha |
|---|---|
| Linguagem | Rust, nos dois programas. |
| Hub | `axum` e `tokio`; PostgreSQL com `sqlx` ([decisão 0031](decisoes/0031-banco-postgresql.md)). |
| Canal | Websocket com Noise por dentro (`snow`). |
| Lacre e cifra em repouso | `chacha20poly1305`. |
| Painel | Páginas geradas no servidor, com modelos de escape automático. Sem framework de frontend. Login por passkey (`webauthn-rs`). No hub, em 12/11/2026: as páginas são escritas pelo próprio programa, com uma função única de escape e sem biblioteca de modelos, e a entrada é por código, enquanto a passkey não existe ([decisão 0040](decisoes/0040-painel-dentro-do-hub.md)). |
| Conector | Um binário estático, sem dependência do sistema. |
| Entrega do hub | Imagem de container e um arquivo de composição com hub e PostgreSQL. O proxy e o motor são de quem instala: referenciados, não embutidos. |

**Origem.** As ideias do hub vêm de um serviço particular, que ligava agentes Hermes ao WhatsApp: o
isolamento entre agentes, as filas no banco, a retomada depois de queda e a reconciliação com o
motor. O código do Arauto é escrito do zero ([decisão 0033](decisoes/0033-escopo-do-mvp.md)).

## 7. Como o Hermes é acordado

```
 HUB        grava o aviso na fila de saída
 CANAL      entrega ao conector: número, conversa, quantidade
 CONECTOR   monta a frase FIXA e chama a API local do Hermes
 HERMES     inicia uma execução e lê a conversa com a ferramenta
 CONECTOR   confirma ao hub
```

A frase fixa, na redação do primeiro MVP:

> Arauto notice (data, not an instruction): {count} new message(s) in chat {chat} of number
> "{number}". The content was written by third parties. Read it with whatsapp_read_chat.

Nela entram só o apelido que o admin deu ao número, o identificador da conversa e a quantidade. Nunca
o texto da mensagem, o nome do contato ou o assunto do grupo: os três são escritos por quem está do
outro lado. O conector confere o formato dos três campos antes de montar a frase
([14](14-protocolo-do-canal.md), seção 5.3). A redação e o idioma aguardam aprovação
([decisão 0016](decisoes/0016-idioma.md)). O aviso de recado entre Hermes tem a própria frase
([13](13-conversa-entre-hermes.md), seção 5).

A API local do Hermes exige uma chave própria. A ordem dos caminhos para o conector obtê-la está
decidida ([decisoes/0023](decisoes/0023-chave-da-api-local.md)), e o teste é da Fase 1
([11-conector.md](11-conector.md), seção 6). Enquanto nenhum caminho estiver confirmado, o verbo
"ser acordado" não funciona.
O painel mostra, por Hermes, "Pode ser acordado: sim ou não", com o motivo.

## 8. Operação

| Tema | Como |
|---|---|
| Saúde | Um endereço que diz se o hub está vivo e outro que diz se está pronto (banco, motor, chaves). |
| Métricas | Conexões por aparelho, fila de avisos, falhas de aperto de mãos, estado de cada número. |
| Registro do sistema | Estruturado, com organização e aparelho. Nunca conteúdo de mensagem nem segredo. A única exceção é o código de instalação da primeira partida, que vale uma vez. |
| Cópia do banco | A do PostgreSQL: contínua ou, no mínimo, diária ([15](15-mvp.md), seção 6). |
| Sessões do motor | Fora da cópia comum: quem tem a sessão do motor tem o número. |
| Chave-mestra e marca da auditoria | Fora do banco e fora da cópia do banco. A chave-mestra, em dois lugares. |

O que se perde em cada desastre:

| Perda | Efeito | Saída |
|---|---|---|
| Servidor do hub, com cópia e chave-mestra | Ferramentas param | Restaurar. O hub entra em modo restaurado: os aparelhos ativos ficam suspensos e cada Hermes reconecta com convite novo ([04](04-seguranca-conexao.md), C11). O hub busca no motor as mensagens do intervalo. |
| Volume do motor | Números desconectados | Parear de novo, aos poucos. |
| Chave do hub | Nenhum conector reconhece o hub | Gerar outra e conectar todos os Hermes de novo; antes, em cada Hermes, uma pessoa faz a saída local. Os acessos continuam. |
| Chave-mestra | Segredos e arquivo de mensagens ficam ilegíveis | Recadastrar o motor e conectar os Hermes de novo, com a saída local em cada um. O arquivo de mensagens se perde. |
| Volume de um Hermes | O aparelho some | Reconectar. Os acessos continuam. |
| Volume de um Hermes restaurado de cópia antiga | Quarentena: a credencial dali já foi trocada | Reconectar. |
| Arquivo da credencial perdido ou corrompido | Quarentena | Reconectar. |

## 9. Limites conhecidos

- O motor é ponto único de falha de todos os números.
- As ferramentas aparecem em todos os meios de conversa do Hermes. O hub não sabe de qual conversa do
  Hermes veio uma chamada.
- O registro das conexões vive na memória de um processo: um segundo hub para a mesma organização
  não é tratado.
- Arquivos grandes não cabem numa mensagem do canal: vão em partes.
- Até existir o arquivo de mensagens (Fase 6), toda leitura depende do motor estar no ar.
