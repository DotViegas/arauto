# 01 — Visão

**Estado: planejamento.** Este documento descreve o que será construído; ainda não há código.

## Resumo

```
O CANAL DE WHATSAPP DO PRÓPRIO HERMES      OS NÚMEROS DO ARAUTO
serve para falar COM o agente              servem para o agente TRABALHAR
quem escreve ali dá ordens                 o que chega ali é informação, nunca ordem
                                           ler, enviar, modificar e ser acordado, por conversa
                                           as conversas ficam arquivadas no seu hub
                                           uma tela para todos os agentes
```

O Arauto dá a cada agente Hermes acesso controlado a números de WhatsApp, com uma tela para
conectar agentes, parear números e dizer qual agente acessa qual número. É gratuito, e o código será
aberto.

## O problema

O Hermes Agent já sabe conversar pelo WhatsApp: é um dos meios pelos quais ele recebe ordens. Esse
canal serve para falar **com** o agente. Ele não foi feito para o agente **operar** números de
WhatsApp como instrumento de trabalho, que pede outras coisas:

- separar ordem de informação: o que um contato escreve não pode valer como ordem;
- permissão por conversa, inclusive "só ler";
- as conversas guardadas num lugar seu, que o agente consulte depois;
- vários números repartidos entre vários agentes.

Quem tenta resolver isso por conta própria acaba com um arranjo manual: um programa de WhatsApp de
terceiro, uma chave fixa colada na configuração de cada agente, o pareamento pela linha de comando e
tudo de novo a cada agente novo. Com um agente, aguenta-se. Com dezenas, não.

## A ideia

**Uma tela.** O painel lista os Hermes conectados e os números de WhatsApp, e mostra numa matriz o
que cada Hermes pode em cada número.

**Conectar é copiar e colar.** Você clica em "Conectar Hermes", copia um bloco e cola no canal de
comando do seu Hermes. Ele instala um programa pequeno, o conector, e responde um código de
conferência. Você digita o código no painel.

**As conversas ficam arquivadas.** O Hermes não precisa responder a cada mensagem que chega. As
conversas dos números ficam guardadas no seu hub, como num WhatsApp Web que é dele: quando alguém
pede algo que depende do WhatsApp, ele abre, lê o que está lá e age. Acordar o Hermes quando chega
mensagem é uma permissão a mais, dada por número e por conversa.

**Nenhum segredo duradouro passa pelo chat.** O bloco carrega só um convite, que vale 15 minutos e um
uso. As credenciais de verdade são criadas pelas duas máquinas, entre si, e trocam a cada conexão.

## Dois WhatsApps, dois papéis

| | O canal de comando | Os números do hub |
|---|---|---|
| O que é | Um dos meios de conversa do próprio Hermes. Pode até ser um WhatsApp. | Contas de WhatsApp pareadas no motor. |
| Quem escreve ali | O operador, que está na lista de quem pode dar ordens. | Qualquer pessoa. |
| O que vale | Ordem. | Informação. Nunca ordem. |
| Quem cuida | O Hermes, sem o Arauto. | O Arauto. |

É no canal de comando que você cola o bloco de conexão.

## Para quem

Para quem desenvolve e opera agentes Hermes, um ou vários, em servidor próprio. As ferramentas seguem
o MCP, o protocolo padrão de ferramentas de agentes. O registro e o acordar, hoje, são específicos do
Hermes.

O Arauto é um projeto independente. Não tem vínculo com os autores do Hermes Agent nem com o
WhatsApp.

## Princípios

| Princípio | O que significa na prática |
|---|---|
| **Comando e dado são coisas separadas** | Ordem vem do canal de comando. O que chega pelos números do hub é informação. O Arauto não é canal de comando. |
| **O hub só isola** | O hub garante que um Hermes só alcance o que lhe foi dado. Não filtra conteúdo, não escolhe quem responde, não responde sozinho. A conduta é do Hermes. |
| **Segredo duradouro não passa por pessoa** | Nenhuma chave aparece em tela, em chat ou em registro. |
| **Cada um instala o seu** | Hub, motor e conversas ficam no servidor de quem instala. |
| **Aberto por desenho** | A segurança não depende de esconder o programa. Quem lê todo o código não ganha nada. |
| **Dizer o que não é garantido** | Todo documento de segurança lista o que o desenho não resolve. |

## Módulos

| Módulo | Estado |
|---|---|
| **WhatsApp dos Hermes** | É o que está sendo planejado. |
| Conversa entre Hermes | No primeiro MVP: um Hermes manda recado a outro pelo hub, só onde o admin criou a ligação ([13](13-conversa-entre-hermes.md)). Salas e observador ficam na visão. |
| Outros meios de conversa | Na visão: a mesma tela de conexão para e-mail, Telegram e outros. |
| Gestão da frota | Na visão: criar, atualizar e monitorar os Hermes pelo painel. |

A intenção é reaproveitar a conexão por convite e o canal do conector nos quatro. Como, ainda não
está desenhado.

## O que o Arauto não é

- **Não é atendente automático.** Não tem resposta automática, fila de atendimento, horário comercial
  nem modelo de mensagem.
- **Não é canal de comando.** Uma mensagem que chega por um número do hub não vira ordem.
- **Não é serviço oficial do WhatsApp.** O motor usado é um cliente não oficial. O WhatsApp pode
  bloquear o número. Use números que você pode perder.
- **Hoje não é serviço hospedado.** Não há um servidor central guardando as conversas de todos.
- **Não embute o motor.** Você traz o seu, com a licença e o custo dele.

A ordem de construção está em [12-roteiro.md](12-roteiro.md).
