# 0020 — Teto técnico de envio

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

Uma LLM em laço pode enviar centenas de mensagens e levar ao bloqueio do número. Um teto no hub evita isso, mas o hub não tem regra de negócio ([0004](0004-o-hub-so-isola.md)).

## Decisão

O hub tem um teto de envio por Hermes e por número, com valor fixo por minuto na primeira versão. Acima dele, o envio é recusado com o erro AR-704. O teto entra como isolamento: protege um número do descontrole de um Hermes, como se protege um vizinho de outro. O hub não olha o conteúdo nem o destinatário.

## Consequências

- O valor de partida, fixado no primeiro MVP, é de 20 envios por minuto ([15-mvp.md](../15-mvp.md)). É calibrado com o uso.
- A alternativa considerada, um teto ajustável pelo admin no painel, não entra na primeira versão.
- O cuidado com o que enviar continua nas instruções de cada Hermes.
