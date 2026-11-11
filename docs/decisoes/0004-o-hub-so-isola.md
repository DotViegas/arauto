# 0004 — O hub só isola

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Seria fácil pôr no hub regras de conduta: horário, palavras proibidas, fila de atendimento, aprovação de envio. Cada uma delas é uma opinião sobre como um agente deve se comportar.

## Decisão

O hub não tem regra de negócio. A única coisa que ele impõe é o isolamento: um Hermes só alcança o que lhe foi dado. A conduta é do Hermes.

## Consequências

- O hub não filtra conteúdo, não responde sozinho e não escolhe qual Hermes age.
- Quando dois Hermes se sobrepõem, o painel avisa e não impede.
- Limites puramente técnicos precisam ser justificados como isolamento ([0020](0020-teto-de-envio.md)).
