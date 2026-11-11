# 0008 — O admin digita o código de conferência

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Quem obtiver o convite antes do uso consegue criar um pedido com o nome certo. Se aprovar fosse um clique diante de dados que o próprio aparelho declara, esse pedido seria aprovado.

## Decisão

O conector calcula e mostra um código de conferência de seis dígitos. O Hermes o diz no canal de comando. O admin o digita no painel. O painel nunca mostra o código.

## Consequências

- Só é aprovado o pedido cujo código o Hermes verdadeiro disse.
- Convite vazado antes do uso rende, no máximo, atrapalhar a conexão.
- O que o aparelho declara aparece como não verificado e não decide nada.
- Cinco erros encerram o pedido; o prazo é de 10 minutos.
- Se o comando rodar de novo, vale o código mais recente.
