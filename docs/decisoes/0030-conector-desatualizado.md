# 0030 — Conector desatualizado: o hub confere a credencial antes de recusar

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

Um conector abaixo da versão mínima é recusado pelo hub (AR-401). No desenho atual ele para de usar o canal e não apresenta mais a credencial. Com isso, um Hermes desatualizado conta como desligado: quem tiver uma cópia dos arquivos e usar um conector atual usa o hub até o admin reconectar esse Hermes, e o uso pode não aparecer como cópia ([04-seguranca-conexao.md](../04-seguranca-conexao.md), seção 6, item 3).

## Decisão

O hub confere a credencial de um conector desatualizado antes de recusar a versão. Se a credencial apresentada já tinha sido trocada, o uso de uma cópia aparece: alerta e quarentena, como em qualquer reuso. A recusa da versão continua.

## Consequências

- A ordem das mensagens está em [14-protocolo-do-canal.md](../14-protocolo-do-canal.md), seção 5.3, como proposta: a "apresentação sem troca". O hub responde ao conector antigo, recebe o que ele apresenta, confere e só então recusa a versão, sem nunca emitir credencial.
- Para isso ficam congeladas, além da primeira mensagem, as mensagens dessa apresentação.
- O conector desatualizado se apresenta na partida do líder e, depois, uma vez a cada 24 horas.
- Até o [04](../04-seguranca-conexao.md) ser atualizado, com aprovação, a seção 6 dele continua descrevendo o limite antigo.
