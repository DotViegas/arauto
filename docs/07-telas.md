# 07 — Telas do painel

**Estado: proposta.** O que cada tela mostra, o que se faz nela e como ela fica vazia ou com erro.

## Resumo

```
Menu:  Hermes · WhatsApps · Acessos · Auditoria · Configurações

T0 Primeiro acesso      criar o primeiro admin e apontar o motor
T1 Hermes               a lista, com o estado de cada um
T2 Conectar Hermes      o bloco e a espera
T3 Confirmar conexão    digitar o código de conferência
T4 WhatsApps            a lista de números
T5 Parear WhatsApp      código de pareamento ou QR
T6 Acessos              a matriz Hermes x número
T7 Auditoria            o que aconteceu, sem conteúdo de mensagem
T8 Admins               quem tem login no painel (em Configurações)
```

## Regras que valem para todas as telas

| Regra | Motivo |
|---|---|
| **O painel é a fonte da verdade, não a fala da LLM.** Cada etapa acende por um fato que o hub observou. | Um Hermes pode dizer "feito" sem ter feito. |
| **Todo texto vindo de aparelho ou do WhatsApp é tratado como hostil**: validado na entrada, escapado na saída. | Nome de máquina, nome de contato e assunto de grupo são escritos por terceiros. |
| **O painel diz o limite da proteção.** Cada Hermes mostra "Proteção básica: a LLM alcança o arquivo da credencial" ou "Proteção blindada". | No modo rápido a LLM alcança o arquivo da credencial. |
| **Toda ação que muda algo pede a passkey na hora.** Com a sessão do painel, sozinha, só se vê. | Uma sessão roubada do navegador não pode bastar para agir. |
| **Convite, QR e código de pareamento aparecem uma vez** e nunca entram em registro. | São segredos de curta duração. |
| **O painel lista o que passa pelo hub.** Outros dispositivos conectados ao mesmo número só aparecem no celular. | O hub não enxerga o que não passa por ele. |

Previsto, com o meio ainda por definir ([decisoes/0025](decisoes/0025-aviso-por-fora-do-painel.md)):
toda aprovação e toda concessão avisarem o admin por fora do painel.

Acima do menu há uma faixa de alertas, visível em qualquer tela: outro aparelho tentou usar um
convite, identidade copiada antes da aprovação, Hermes em quarentena, conexão tomada por outra,
encerramento pedido pelo aparelho, reuso depois do encerramento, três trocas sem confirmação em 24
horas, rede ou país novo, pedido aguardando código, motor indisponível, configuração do motor
corrigida pelo hub, número desconectado, hub restaurado de cópia.

A sessão do painel dura 12 horas, ou 30 minutos sem uso. A tela de espera se atualiza sozinha, por
consulta periódica ao hub.

## T0. Primeiro acesso

- **Mostra:** três passos. (1) O código de instalação, que o hub mostra no registro do sistema na
  primeira partida, e o cadastro da passkey do primeiro admin. (2) Os endereços do hub: o do painel e
  o dos conectores, que vai em todo bloco. (3) O motor: endereço e chave da API, com botão "Testar".
- **Regra:** sem o código de instalação ninguém cria o primeiro admin. Ele vale uma vez e por pouco
  tempo. Criado o admin, o código deixa de valer; um novo só sai por comando no servidor, para
  quando o único admin perde a passkey.
- **Erros:** código de instalação errado; motor não responde, chave recusada ou versão não testada. O
  passo 3 pode ficar para depois.

## T1. Hermes

- **Mostra:** contadores ("5 conectados, 1 aguardando, 0 em alerta") e uma linha por Hermes: nome,
  estado ([10](10-maquinas-de-estado.md), seção 3), números com acesso, se pode ser acordado, versão
  do conector, visto por último.
- **Ficha do Hermes:** o limite da proteção; endereço observado, atual e anterior; o que o aparelho
  declarou, marcado como não verificado; data da conexão; avisos na fila; últimos eventos. Texto
  fixo: "As ferramentas aparecem em todos os meios de conversa deste Hermes. Quem pode dar ordens a
  ele pode usar os números concedidos."
- **Ações:** Conectar Hermes; Acessos; Reconectar; Renomear; Revogar (pede o nome digitado);
  Excluir (só sem convite emitido e sem aparelho pendente, aprovado, ativo, suspenso ou em
  quarentena).
- **Vazio:** "Nenhum Hermes conectado. Você cola um bloco no canal de comando do Hermes e digita aqui
  o código de conferência que ele responder."
- **Quarentena:** "O hub deixou de confiar na credencial de {nome}. Motivo: uma credencial já
  trocada foi apresentada | duas conexões ao mesmo tempo | credencial ilegível | o último uso não
  foi reconhecido no reconectar. O acesso está parado." Saídas: Reconectar ou Revogar. Não existe "liberar".
- **Conexão tomada:** "A conexão de {nome} foi tomada por outra, vinda de {endereço}." Aparece
  sempre que o hub dá por morta uma sessão que estava servindo e outra entra no lugar.
- **Encerrado pelo próprio aparelho:** "{nome} pediu o próprio encerramento, de {endereço}. Último
  uso: {data e hora}, de {endereço}."
- **Reuso depois do encerramento:** para aparelho revogado, substituído ou vencido por inatividade
  que apresenta uma credencial antiga: "Uma cópia de {nome} foi usada antes do encerramento."
- **Hub restaurado:** uma faixa em todas as telas: "O hub voltou de uma cópia de {data}. O que foi
  aprovado, revogado, reconectado ou mudado nos acessos depois disso se perdeu. Reconecte cada Hermes
  e confira os acessos e a lista de admins." Os Hermes com aparelho ativo aparecem como suspensos,
  cada um com "Reconectar" e "Revogar". Não existe "reativar". Os aparelhos que apresentaram um
  lacre válido que o banco não explica aparecem numa lista à parte, com hora, endereço e o que
  declararam.

## T2. Conectar Hermes

```
Conectar Hermes: Ana                  O convite vale por mais 14:32 · uso único
┌──────────────────────────────────────────────────────────────────┐
│ (o bloco, somente leitura)                                         │
└──────────────────────────────────────────────────────────────────┘
[ Copiar bloco ]    O que este bloco faz?    Cancelar convite

1. Cole no canal de comando do Hermes "Ana". Em nenhum outro lugar.
2. Ele vai responder um código de conferência de 6 dígitos. Volte aqui.

[x] Convite emitido               14:02:11
[ ] Um aparelho usou o convite    aguardando...
[ ] Conector no ar
[ ] Código confirmado por você
[ ] Primeira credencial confirmada
```

- **As etapas:** "Conector no ar" acende na primeira conexão de canal do aparelho, em espera ou já
  na troca. Ela e a do código acendem em qualquer ordem.
- **Nome:** único, 3 a 32 caracteres, só letras, números, espaço e hífen. Nome curto e sem símbolos
  reduz o que ele consegue carregar para dentro do bloco.
- **O bloco só existe nesta tela e nesta aba.** O hub não guarda o convite. Recarregar mostra "O
  convite vale por mais mm:ss, mas o bloco não pode ser exibido de novo".
- **Não existe "enviar por e-mail" nem "enviar pelo WhatsApp".** Um bloco que chegasse ao Hermes por
  um número do hub chegaria como dado, e deve ser recusado.
- **Se o Hermes pedir aprovação para rodar o comando,** quem aprova é você, no canal de comando.
- **Ajuda por tempo:** 90 segundos sem inscrição abrem "Nada chegou ainda", com as três causas: o
  Hermes está esperando a sua aprovação; o Hermes não alcança o hub; o bloco foi colado em outro
  lugar.

## T3. Confirmar conexão

```
Um aparelho usou o convite de "Ana"                   confirme em 09:41

Visto pelo hub        endereço 203.0.113.7 · 14:03:02
Dito pelo aparelho    máquina hermes-ana · conector 1.0.0 · Hermes 0.22.5
                      (o aparelho pode mentir aqui; o que decide é o código)

Digite o código de conferência que o Hermes respondeu no canal de comando:
        [ _ _ _ ] - [ _ _ _ ]

[ Aprovar ]      [ Negar ]      Não recebi código nenhum
```

- **O painel não exibe o código de conferência.**
- **Aprovar** só com o código certo, e pede a passkey.
- **"Não recebi código nenhum":** "Se o Hermes não respondeu um código, não aprove. Se ele respondeu
  AR-102, outro aparelho usou o convite: negue e gere outro."
- **Estados:** código errado ("tentativa 2 de 5"); prazo de 10 minutos esgotado; outro aparelho
  tentou o mesmo convite (alerta, e o pedido atual continua valendo); o Hermes rodou o comando de
  novo ("vale o último código que ele respondeu").
- **No reconectar,** antes do código: "Último uso do aparelho atual: {data e hora}, do endereço
  {endereço}. Você reconhece?" Se o Hermes esteve desligado e houve uso, alguém usou uma cópia.
  "Não reconheço" gera o alerta de cópia, põe o aparelho antigo em quarentena se ele estava ativo
  (suspenso ou em quarentena, fica como está), mostra as chamadas dele desde então e deixa a
  aprovação seguir.
- **Depois de aprovar:** "Ana foi aprovado. Assim que o conector confirmar, aparece como conectado."
  Numa primeira conexão, segue "Ele ainda não tem acesso a número nenhum", com o botão "Dar acesso".
  Num reconectar, segue "Os acessos que Ana já tinha voltam a valer para este aparelho", com o botão
  "Ver acessos".

## T4. WhatsApps

- **Mostra:** uma linha por número: apelido, número pareado, estado, se é "de comando", Hermes com
  acesso, hora da última mensagem recebida (sem conteúdo).
- **Ações:** Adicionar; Parear de novo; Reiniciar a sessão do motor; Desconectar do WhatsApp; Marcar
  como "de comando"; Remover (pergunta o destino do arquivo de mensagens: manter por 30 dias ou
  apagar agora). Desconectar e Remover pedem o apelido digitado, além da passkey.
- **Primeiro número:** aceite registrado na auditoria: "O motor é um cliente não oficial do
  WhatsApp. O WhatsApp pode bloquear o número. Use números que você pode perder."
- **Erros:** "Motor indisponível: o hub não fala com o motor desde HH:MM. Os Hermes continuam
  conectados e leem o que já está no arquivo de mensagens; não enviam nem recebem mensagens novas.";
  "Desconectado: o dispositivo conectado foi removido no celular"; "Este número envia, mas o hub não
  está recebendo as mensagens dele",
  com o botão Reconfigurar.

## T5. Parear WhatsApp

- **Passo 1:** apelido e número com código do país.
- **Passo 2:** código de pareamento (com o caminho no celular e a validade) ou QR (que se renova
  sozinho).
- **Passo 3:** "Pareado: +00 00 00000-0000." Se o número pareado for outro: "O celular que pareou é
  de um número diferente do informado. Manter este | Desfazer." Enquanto o admin não decide, o estado
  é "número trocado" e os acessos ficam parados.

## T6. Acessos

Nas linhas ficam os Hermes; nas colunas, os números.

```
Acessos           [ por Hermes | por número ]          filtro: ________

                  Balcão            Compras           Portaria
                  +00 ..0001        +00 ..0002        +00 ..0003
Hermes Ana        L E - A  todas    - - - -           L E - A  todas
Hermes Bento      L - - -  3 conv.  L E M A  todas    - - - -
Hermes Antigo     em quarentena (acessos preservados)

L ler · E enviar · M modificar · A ser acordado
(!) Balcão: 2 Hermes são acordados nas mesmas conversas
```

- **Célula:** escopo (todas | somente conversas escolhidas), os quatro verbos, "pode iniciar
  conversa", linhas de conversa. A tela escreve a frase resultante: "O Hermes Bento pode ler em 3
  conversas do número Balcão. Não envia, não modifica e não é acordado."
- **Salvar** vale na chamada seguinte, sem reiniciar nada.
- **Avisos que não bloqueiam:** dois Hermes acordados ou enviando nas mesmas conversas; linha de
  conversa restritiva sob o escopo "todas"; **enviar** concedido num número marcado como "de
  comando".
- **Vazios:** "Conecte um Hermes primeiro"; "Adicione um WhatsApp primeiro"; "Ninguém tem acesso a
  nada. Clique numa célula para conceder."

## T7. Auditoria

- **Mostra:** linha do tempo, da mais recente para a mais antiga: quando, quem (admin, Hermes,
  sistema), o quê, onde (Hermes, número, conversa), resultado, código AR, endereço.
- **Eventos:** convite emitido, usado, vencido, cancelado e recusado (AR-101); tentativa de outro
  aparelho; código de
  conferência certo e errado; aprovação e negação; conexão aberta e fechada; credencial trocada;
  conexão que terminou sem confirmar a troca; conexão tomada por outra; quarentena; revogação;
  encerramento pedido pelo aparelho; reuso depois do encerramento; uso não reconhecido;
  duas conexões em espera com a mesma identidade; rede ou país novo; aparelho substituído; acesso
  alterado, com antes e depois; login; admin adicionado e removido; passkey criada e removida;
  número adicionado, pareado,
  removido; chamada de ferramenta; negação por falta de verbo; aviso confirmado, expirado e
  descartado; configuração do motor corrigida pelo hub; hub restaurado de cópia; aparelho suspenso;
  aparelho desconhecido com lacre válido.
- **Nunca contém:** conteúdo de mensagem, convite, credencial, QR, código de pareamento.
- **Ações:** filtrar, abrir o detalhe, exportar (pede a passkey). Não há editar nem apagar.

## T8. Admins

Fica em Configurações.

- **Mostra:** os admins e as passkeys de cada um, com a data de criação e a do último uso.
- **Ações:** adicionar admin (o novo cadastra a própria passkey), remover admin, remover passkey.
  Todas pedem a passkey de quem faz. O último admin não pode ser removido.
- **Depois de o hub voltar de uma cópia,** a faixa manda conferir esta lista: um admin removido
  depois da cópia volta a aparecer aqui.
