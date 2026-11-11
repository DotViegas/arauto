# 0021 — Prazo de inatividade

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

O uso de uma cópia da credencial só é descoberto quando o Hermes verdadeiro volta. Um Hermes desligado para sempre e nunca revogado nunca volta.

## Decisão

Um aparelho ativo que fica 30 dias sem conectar passa a vencido e só volta com Reconectar, por convite novo. Os acessos do Hermes ficam guardados.

## Consequências

- Pega a cópia usada pela primeira vez depois do prazo.
- Não pega a cópia que já está em uso: ela conecta como o aparelho, e o prazo nunca vence.
- A alternativa que fecharia o caso inteiro, uma validade máxima do aparelho, não foi adotada: custaria reconectar de tempos em tempos.
- Um aparelho vencido por inatividade que apresenta uma credencial mais antiga do que a última gera o alerta de reuso depois do encerramento, e o reconectar mostra o último uso dele ([04-seguranca-conexao.md](../04-seguranca-conexao.md), C7 e C10). Sem isso, o uso de uma cópia deixaria de aparecer depois do prazo.
- O aparelho vencido por inatividade recebe o erro AR-507 ([11-conector.md](../11-conector.md), seção 8). Falta escrever o rótulo dele no painel.
