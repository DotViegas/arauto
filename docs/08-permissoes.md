# 08 — Permissões

**Estado: proposta.** O que um Hermes pode fazer em cada número.

## Resumo

```
L  LER            ver conversas e mensagens, buscar, baixar arquivo
E  ENVIAR         escrever, reagir, marcar como lida e o que mais cria atividade nova
M  MODIFICAR      editar e apagar o que já existe
A  SER ACORDADO   o hub avisa o Hermes quando chega mensagem
```

Três regras valem sempre:

1. **Conectar não é permitir.** Todo Hermes nasce sem acesso a número nenhum.
2. **Ausência é negação.** O que não foi marcado não existe para o Hermes.
3. **Quem decide é o hub**, a cada chamada e a cada aviso. O conector não decide nada.

## 1. Os quatro verbos

| Verbo | Libera | Não libera |
|---|---|---|
| **Ler** | Listar conversas, ler mensagens, buscar, baixar arquivo, ver participantes de grupo, ver dados públicos do contato, ver o estado do número. | Marcar como lida. |
| **Enviar** | Texto, arquivo, resposta, reação, "digitando", marcar como lida, encaminhar e conferir se um telefone tem WhatsApp. | Iniciar conversa com quem nunca falou com o número, salvo com a opção "pode iniciar conversa". |
| **Modificar** | Editar e apagar mensagem. | Perfil, privacidade e bloqueios da conta. |
| **Ser acordado** | O hub avisa o Hermes quando chega mensagem numa conversa do escopo. | O conteúdo da mensagem dentro do aviso. |

- Conferir se um telefone tem WhatsApp fica em **enviar** porque pergunta ao WhatsApp sobre alguém de
  fora das conversas, e só faz sentido para quem vai escrever.
- **Ser acordado** exige **ler** na mesma linha.
- **Pode iniciar conversa** exige **enviar** e escopo "todas".
- Não existe verbo que dê a um Hermes criar, parear, reiniciar, desconectar ou remover um número. Isso
  é só do admin, no painel.

## 2. Escopo e linha de conversa

| Escopo | Significado |
|---|---|
| **Todas as conversas** | Os verbos valem para toda conversa do número, inclusive as que surgirem. |
| **Somente conversas escolhidas** | Só as conversas listadas existem para aquele Hermes. |

Uma **linha de conversa**, quando existe, substitui por inteiro os verbos do número naquela conversa.
Uma linha de conversa com os quatro verbos desligados equivale a bloquear.

Para isolar de verdade, prefira "somente conversas escolhidas". Uma lista de permissão falha fechada:
o que não está nela não existe. Uma linha de conversa restritiva sob "todas" falha aberta se o
identificador da conversa mudar no motor.

## 3. Regra de decisão

Aplicada no hub, nesta ordem:

1. O aparelho do Hermes não está ativo (pendente, aprovado, suspenso, em quarentena, vencido,
   revogado) ou o conector é incompatível: **nega**.
2. Não existe acesso entre o Hermes e o número: **nega**.
3. Existe linha da conversa: valem os verbos dela, e só eles.
4. Não existe linha da conversa: com escopo "todas", valem os verbos do número; com escopo "lista",
   **nega**.
5. Enviar para conversa que ainda não existe: exige escopo "todas", **enviar** e "pode iniciar
   conversa".
6. A chamada depende do motor (enviar, modificar, conferir um telefone, baixar um arquivo que não
   está guardado) e o número está indisponível ou o motor está fora do ar: **erro**. Ler o que já
   está no arquivo de mensagens não depende do motor.

A mudança vale na chamada seguinte. Um aviso que já está na fila é reavaliado na entrega e
descartado se o acesso foi retirado.

### O que o Hermes vê em cada caso

| Situação | Resposta |
|---|---|
| Aparelho em quarentena, revogado ou suspenso; conector incompatível | AR-501, AR-502, AR-506 ou AR-401 |
| Não há acesso ao número, ou falta o verbo no número | AR-701 |
| A conversa está fora do escopo, ou a linha da conversa não tem o verbo | AR-705, a mesma resposta de conversa inexistente |
| Número desconectado ou em "número trocado", numa chamada que depende do motor | AR-702 |
| Motor fora do ar, numa chamada que depende do motor | AR-703 |
| Teto de envio atingido | AR-704 |
| Na conversa entre Hermes: teto de recado atingido; sem par com esse nome; fio não encontrado | AR-706, AR-707, AR-708 ([13](13-conversa-entre-hermes.md)) |

O Hermes fica sabendo que não tem um verbo num número. Não fica sabendo que existe uma conversa fora
do escopo dele.

## 4. Ser acordado

O aviso é um ponteiro, com frase fixa ([03-arquitetura.md](03-arquitetura.md), seção 7). O que o
Hermes faz ao acordar é definido pelas instruções do próprio Hermes, não pelo hub.

| Regra de entrega | Valor |
|---|---|
| Garantia | Pelo menos uma vez, com confirmação do conector. Aviso repetido é descartado pelo conector. |
| Fila enquanto o Hermes está fora | 24 horas ou 1.000 avisos. Passando de 1.000, o mais antigo expira. |
| Agrupamento | Um aviso pendente por conversa: o que chega depois soma na quantidade. Depois de um aviso entregue, o seguinte da mesma conversa só sai em 10 segundos. |
| Teto do conector | 30 acordadas por minuto e 300 por hora, somando os tipos. Acima disso o conector recusa, e o hub tenta de novo em 5 minutos. |
| Mensagem do próprio número | Não acorda ninguém. Evita laço. |

O aviso de recado entre Hermes segue as mesmas regras, com os tetos de
[13-conversa-entre-hermes.md](13-conversa-entre-hermes.md), seção 6.

O painel mostra, por Hermes, se ele pode ser acordado. Quando não pode, o verbo fica desabilitado, com
o motivo. Enquanto a chave da API local do Hermes não estiver resolvida
([11-conector.md](11-conector.md), seção 6), nenhum Hermes pode.

## 5. O que o hub não decide

- Filtrar por conteúdo, palavra, horário ou tipo de contato.
- Escolher qual Hermes responde.
- Responder sozinho.
- Segurar mensagem para aprovação.

Quando dois Hermes podem enviar ou ser acordados nas mesmas conversas, o painel **avisa e não
impede**.

## 6. Dois cuidados que não são permissão

**As ferramentas aparecem em todos os meios de conversa do Hermes.** Quem consegue dar ordens ao
Hermes, por qualquer meio de conversa dele, consegue usar os números concedidos. Restringir
ferramentas por meio de conversa é configuração do Hermes, não do hub. O painel diz isso na ficha de
cada Hermes.

**Número que também é canal de comando.** Se um número pareado no motor estiver na lista de quem dá
ordens a algum Hermes, um Hermes com **enviar** naquele número consegue escrever no canal de comando
do outro. O hub não conhece essas listas. O painel permite marcar um número como "de comando" e
alerta ao conceder **enviar** nele.

## 7. Teto de envio

O hub tem um teto técnico de envio por Hermes e por número, para um laço da LLM não levar ao
bloqueio do número ([decisoes/0020](decisoes/0020-teto-de-envio.md)). O valor é fixo na primeira
versão: 20 envios por minuto, como ponto de partida. Acima dele, o envio é recusado com AR-704. É isolamento, como
proteger um vizinho de outro: o hub não olha o conteúdo nem o destinatário.
