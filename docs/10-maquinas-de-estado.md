# 10 — Máquinas de estado

**Estado: proposta.** Os estados de cada coisa, quem muda cada um e os prazos. Os números são pontos
de partida, a calibrar no protótipo.

## Resumo

```
CONVITE     emitido -> usado | vencido | cancelado
APARELHO    pendente -> aprovado -> ativo
            saídas: negado, vencido, quarentena, suspenso, substituído, revogado
NÚMERO      sem pareamento -> pareando -> conectado <-> reconectando
            desvios: desconectado, número trocado, falha, removido
AVISO       pendente -> enviado -> confirmado
            saídas: expirado, descartado
```

Todo prazo usa só o relógio do hub.

## 1. Convite

| De | Para | Quem dispara |
|---|---|---|
| (nada) | **emitido** | Admin clica "Conectar Hermes" ou "Reconectar". |
| emitido | **usado** | Primeira inscrição válida. |
| emitido | **vencido** | 15 minutos sem inscrição. O hub apaga o resumo do convite. |
| emitido | **cancelado** | Admin cancela, gera outro convite para o mesmo Hermes ou revoga o Hermes; ou o hub entra em modo restaurado. |

- Um Hermes tem no máximo um convite emitido por vez.
- Estados finais ficam gravados. Um convite nunca volta a valer.
- No hub, o resumo também é apagado quando o convite é cancelado, e não só quando vence
  ([14](14-protocolo-do-canal.md), seção 12). Um convite vencido que uma inscrição encontra é
  encerrado ali mesmo.
- Convite usado, apresentado por **outra identidade** enquanto o pedido está pendente: recusa e
  **alerta** no painel.
- Em qualquer outro caso de convite que não vale: recusa e só registro na auditoria. **Nunca derruba
  um Hermes conectado.**

## 2. Aparelho

| De | Para | Quem dispara |
|---|---|---|
| (nada) | **pendente** | Inscrição válida ([04](04-seguranca-conexao.md), C3). |
| pendente | **aprovado** | Admin digita o código de conferência certo. |
| pendente | **negado** | Cinco códigos errados; a sexta inscrição; o admin nega, gera outro convite para o mesmo Hermes ou revoga o Hermes; o próprio aparelho desiste (`leave`); ou duas conexões em espera com a mesma identidade. |
| pendente | **vencido** | 10 minutos sem código, contados da primeira inscrição; ou o hub entra em modo restaurado. |
| aprovado | **ativo** | A primeira credencial foi confirmada. |
| aprovado | **vencido** | 24 horas sem confirmar a primeira credencial; ou o hub entra em modo restaurado. |
| aprovado, ativo | **quarentena** | O hub deixou de confiar na credencial: reuso, conexão dupla ou credencial ilegível; ou o admin não reconhece o último uso de um aparelho ativo ao aprovar um reconectar. |
| ativo | **suspenso** | O hub entrou em modo restaurado ([04](04-seguranca-conexao.md), C11). |
| ativo | **vencido** | 30 dias sem conectar ([decisoes/0021](decisoes/0021-prazo-de-inatividade.md)). |
| ativo, suspenso, quarentena | **substituído** | Um aparelho novo do mesmo Hermes teve a primeira credencial confirmada (Reconectar). |
| aprovado, ativo, suspenso, quarentena | **revogado** | O admin revoga o Hermes ou o exclui; o admin gera outro convite e o aparelho ainda não tinha credencial confirmada; ou o próprio aparelho pede o encerramento (`leave`, ou ao ser trocado por outro na mesma máquina). Encerramento pedido pelo aparelho gera alerta. |

- Um Hermes tem no máximo um aparelho em uso (o que já confirmou credencial: ativo, suspenso ou em
  quarentena) e um pedido em andamento (o que ainda não confirmou: pendente ou aprovado, mesmo que
  tenha ido à quarentena).
- Negado, vencido, substituído e revogado são finais. A linha fica, como registro: se a mesma
  identidade tentar voltar, o hub reconhece e responde a situação dela.
- Da quarentena e do suspenso só se sai por Reconectar (vira substituído) ou Revogar. Não existe
  "liberar" nem "reativar".
- "Incompatível" não é estado do aparelho. É um rótulo calculado a partir da versão do conector
  gravada na última conexão aceita.

No hub, em 11/11/2026:

- **Prazo vencido vale na hora.** Quem encontra um prazo vencido antes da varredura o aplica ali:
  a inscrição, a aprovação e as duas gravações da troca. As da troca aplicam também os 30 dias de
  inatividade.
- **Negar** pelo comando de administração só vale para pedido pendente. Um aprovado sem credencial
  sai por revogar ou por convite novo.
- **Excluir um Hermes** encerra tudo o que é dele, como um revogar, e tira as ligações dele
  ([13](13-conversa-entre-hermes.md), seção 7).
- **Vencido sem motivo que explique** responde AR-507, o único código que nunca leva o conector a
  descartar nada.

## 3. Hermes: o que a tela mostra

O rótulo da linha é calculado a partir do aparelho e do canal.

| Rótulo | Quando |
|---|---|
| Sem aparelho | A linha existe, sem convite emitido e sem aparelho vivo. |
| Aguardando o Hermes | Há convite emitido. |
| Aguardando seu código | Há aparelho pendente. |
| Aprovado, aguardando o conector | Aparelho aprovado, ainda sem a primeira credencial confirmada. |
| Conectado | Aparelho ativo com sessão do canal aberta. |
| Desconectado há X | Aparelho ativo sem sessão do canal. |
| Incompatível | O conector está abaixo da versão mínima. |
| Em quarentena | O hub deixou de confiar na credencial. |
| Suspenso | O hub voltou de uma cópia de segurança. Este Hermes precisa ser reconectado. |
| Revogado | O aparelho foi encerrado. |

Os acessos pertencem à linha do Hermes e valem para o aparelho ativo. Trocar de aparelho não mexe
neles. Com o aparelho em qualquer outro estado, toda chamada é negada.

O rótulo do aparelho vencido por inatividade ainda não está escrito
([decisoes/0021](decisoes/0021-prazo-de-inatividade.md)). Pelas regras acima, a linha aparece como
"Sem aparelho".

## 4. Número

| De | Para | Quem dispara |
|---|---|---|
| (nada) | **sem pareamento** | Admin adiciona o número. |
| sem pareamento, desconectado | **pareando** | Admin pede o código de pareamento ou o QR. |
| pareando | **conectado** | O celular confirmou, e o número pareado é o esperado. |
| pareando | **número trocado** | O celular confirmou, mas o número pareado é outro. |
| pareando | **sem pareamento** | O código de pareamento ou o QR venceu sem uso. |
| conectado | **reconectando** | O motor perdeu a ligação com o WhatsApp. |
| reconectando | **conectado** | A ligação voltou. |
| conectado, reconectando | **desconectado** | Alguém removeu o dispositivo conectado, no celular, ou o admin desconectou pelo painel. |
| número trocado | **conectado** | O admin confirma o número novo. |
| número trocado | **sem pareamento** | O admin desfaz o pareamento. |
| conectado, reconectando | **falha** | O motor informou erro na sessão do motor. |
| falha | **reconectando** | O admin reinicia a sessão do motor. |
| qualquer | **removido** | O admin remove o número. |

- Em **número trocado** os acessos ficam parados até o admin decidir.
- **Estado desconhecido** não é estado do número: é o que a tela mostra quando o motor não responde.
- O hub confere a configuração de cada sessão do motor na partida e a cada 5 minutos. Se achar algo
  diferente do esperado (por exemplo, um destino de eventos que não é o hub), corrige e alerta.

## 5. Aviso

| De | Para | Quem dispara |
|---|---|---|
| (nada) | **pendente** | Chegou mensagem numa conversa em que o Hermes pode ser acordado, ou recado de um par que o acorda ([13](13-conversa-entre-hermes.md)). |
| pendente | **enviado** | O hub entregou ao conector. |
| enviado | **confirmado** | O conector confirmou depois de a API local do Hermes aceitar. |
| enviado | **pendente** | O conector não confirmou no prazo, ou respondeu que falhou ou que atingiu o teto dele; o hub tenta de novo. |
| pendente, enviado | **expirado** | 24 horas na fila, ou a fila do Hermes passou de 1.000 e este era o mais antigo. |
| pendente, enviado | **descartado** | O acesso ou a ligação saiu antes da confirmação, ou o conector recusou o aviso por estar fora do formato. |

- A entrega é pelo menos uma vez. O conector descarta aviso repetido pelo identificador.
- Só existe um aviso pendente por alvo (a conversa, ou o Hermes remetente): o que chega depois soma
  na quantidade. Depois de um aviso entregue, o seguinte do mesmo alvo só sai em 10 segundos.
- Mensagem enviada pelo próprio número não acorda ninguém.

## 6. Conexão e sessão do canal

Uma conexão de canal começa no aperto de mãos. A **sessão do canal** começa quando a troca da
credencial é confirmada ([04](04-seguranca-conexao.md), C7). Antes disso a conexão só serve para a
espera do pedido ou para a própria troca. As conexões curtas do `join` e do `leave` não são de
canal ([04](04-seguranca-conexao.md), C9).

| Motivo do fim | O que o conector faz |
|---|---|
| Queda de rede, ou falta de batimento | Reconecta na hora. Se a tentativa falhar, abre outra, com espera crescente e sorteada, de 1 a 60 segundos. |
| Dada por morta (o hub aceitou outra conexão no lugar; AR-208) | Reconecta na hora. Se alguém tomou o lugar dele, é aí que aparece. |
| O hub está parando ou sem recursos (AR-205) | Reconecta depois de uma espera sorteada. |
| Tentativa trocada por outra mais nova do mesmo aparelho (AR-206) | Espera de 5 a 15 segundos, sorteados, e tenta de novo. |
| Erro de protocolo ou limite (AR-204) | Reconecta com espera crescente e não repete a mesma mensagem. |
| O "gravei" da troca de 24 horas não chegou em 60 segundos | Reconecta e apresenta a credencial que tem. |
| Aparelho substituído (AR-503) | Passa a usar a pasta do aparelho novo. Se não tem pasta nova, para de usar o canal e mostra o erro. |
| Pedido negado ou vencido | Se a pasta daquele pedido ainda não tem credencial, descarta a pasta e, se não há outro aparelho, remove o registro. Se tem, não descarta nada e mostra o erro. |
| Incompatível | Para de usar o canal e mostra o erro de versão em toda ferramenta. |
| Quarentena, suspenso | Para de usar o canal e mostra o erro. Não apaga nada. |
| Revogado | Para de usar o canal e mostra AR-502. Não apaga nada. |
| O hub não reconhece o aparelho (AR-504) | Para de usar o canal e mostra o erro. Não apaga nada. |
| Vencido por inatividade (AR-507) | Para de usar o canal e mostra o erro. Não apaga nada. |

"Parar de usar o canal" não encerra o conector: ele continua respondendo ao Hermes, com o erro, e
continua atendendo o `join` e o `leave` pelo soquete local.

## 7. Prazos e limites

| Item | Valor |
|---|---|
| Validade do convite | 15 minutos |
| Prazo para digitar o código de conferência | 10 minutos a partir da primeira inscrição |
| Tentativas de código | 5 |
| Inscrições por pedido | 5 |
| Aprovado sem confirmar a primeira credencial | 24 horas |
| Batimento do canal | A cada 30 segundos; 90 segundos sem resposta encerram a conexão |
| Conferência de quem apresenta uma credencial | 5 segundos |
| Teste da conexão antiga numa conexão dupla | 3 segundos |
| Conexão que ainda não virou sessão, fora a espera do pedido | 90 segundos |
| Troca de credencial dentro da sessão | A cada 24 horas |
| Prazo do "gravei", em qualquer troca | 60 segundos |
| Conexões sem confirmar a troca que geram alerta | 3 em 24 horas |
| Fila de avisos por Hermes | 24 horas ou 1.000 avisos |
| Intervalo entre avisos do mesmo alvo | 10 segundos |
| Teto de acordadas do conector | 30 por minuto e 300 por hora |
| Varredura do hub: aparelhos conectados e filas | A cada 2 segundos, por um conjunto de conexões do banco só dela |
| Um quadro que não sai do hub | 30 segundos fecham a conexão |
| Parada em ordem do hub | Até 8 segundos pelas chamadas em andamento, e mais 2 para os últimos `closing` |
| Conferência da configuração do motor | A cada 5 minutos |
| Revogação: tempo até o canal cair | Menos de 5 segundos |
| Inatividade que exige reconectar | 30 dias |
| Teto de envio por Hermes e por número | 20 por minuto |

Os limites do canal, por fase e por origem, estão em [14-protocolo-do-canal.md](14-protocolo-do-canal.md),
seção 4. Os tetos de recado, em [13-conversa-entre-hermes.md](13-conversa-entre-hermes.md), seção 6. Os
prazos de guarda, em [15-mvp.md](15-mvp.md), seção 2. A definir: o prazo padrão de guarda do arquivo
de mensagens.
