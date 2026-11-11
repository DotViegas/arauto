# 0012 — Canal cifrado por dentro do websocket

**Estado:** proposta, a validar no protótipo · **Data:** 09/11/2026

## Contexto

Em instalações reais o TLS termina num proxy que não é do projeto, e numa rede interna ele pode não existir. Depender só do TLS deixaria a segurança da conexão nas mãos de cada instalação.

## Decisão

O canal usa o padrão Noise XK por dentro do websocket, numa suíte fixa e sem modo de reserva. A chave estática do conector é a identidade do aparelho; a do hub é a que veio no bloco. Na inscrição o conector leva o convite; nas conexões seguintes, a credencial lacrada.

## Consequências

- A segurança do canal é a mesma com TLS, sem TLS e com proxy de terceiro.
- O convite só viaja cifrado, para o hub verdadeiro. O hub guarda só o resumo dele.
- O conector prova que tem a identidade já na inscrição.
- Um só aperto de mãos para inscrição e conexão.
- O código de conferência sai do próprio aperto de mãos e de um número sorteado pelo hub: os dois lados calculam e ninguém o transmite.
- Não protege o painel: o navegador do admin depende do TLS de quem instala.
- Custo: implementar e testar o formato das mensagens, com vetores de teste.
- **Alternativa registrada:** TLS 1.3 com desafio assinado. É mais simples e basta quando o proxy é de quem instala. Fica como plano B se o protótipo mostrar que o custo do Noise não compensa.
