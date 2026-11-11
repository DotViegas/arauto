# 0009 — Caixa preta no hub e credencial lacrada

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Depois de conectado, o Hermes precisa voltar sozinho quando reinicia. Para isso ele tem de guardar algo. Uma chave fixa guardada serve para sempre a quem a copiar.

## Decisão

Quem gera é o hub, numa parte chamada caixa preta, por sorteio do sistema. O Hermes guarda uma credencial lacrada, que não consegue abrir. Toda conexão começa trocando a credencial por outra, e a sessão do canal só serve depois de a troca ser confirmada. Usar uma cópia leva à quarentena.

## Consequências

- O Hermes conecta uma vez e volta sozinho.
- Para usar o hub é preciso trocar a credencial, e trocar mata a anterior. Por isso quem usa uma cópia é descoberto quando o Hermes verdadeiro conecta.
- Com o Hermes verdadeiro ligado, a cópia é barrada em segundos. Com ele desligado, a cópia funciona até ele voltar: não há limite de tempo.
- Uma cópia antiga não dá acesso, mas apresentá-la põe o aparelho em quarentena. A pasta do conector deve ficar fora das cópias de segurança.
- Da quarentena só se sai com convite novo. Não existe "liberar".
- Depois da primeira confirmada, o hub nunca emite credencial só pela identidade.
- A chave-mestra da caixa preta fica fora do banco e é um dos itens a proteger.
