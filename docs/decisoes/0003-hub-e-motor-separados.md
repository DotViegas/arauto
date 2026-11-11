# 0003 — Hub e motor são processos separados

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

O motor de WhatsApp é um programa de terceiro. Fundi-lo ao hub obrigaria a reiniciar todas as sessões do motor a cada versão nova do hub. Deixá-lo solto, configurado à mão, permite erro silencioso: uma sessão do motor criada com a opção errada descarta histórico sem avisar.

## Decisão

Hub e motor continuam dois processos. O hub é o único que fala com o motor e é o dono da configuração de cada sessão do motor: cria, configura, confere e corrige.

## Consequências

- Uma versão nova do hub não derruba o WhatsApp.
- A configuração de uma sessão do motor existe num lugar só.
- Trocar a versão do motor continua sendo um ato de quem instala. O hub não controla containers.
- O hub precisa de um teste de contrato contra o motor a cada versão nova.
