# 0007 — Convite de uso único, executado pela LLM do Hermes

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Conectar um agente exigia acesso de administrador ao servidor. A alternativa simples, uma chave fixa colada no chat, deixaria um segredo permanente no histórico da conversa.

## Decisão

Conecta-se um Hermes colando um bloco no canal de comando dele. O bloco carrega um convite que vale 15 minutos e um uso, e que só serve para pedir conexão. Quem executa é a LLM do Hermes.

## Consequências

- Nenhum segredo duradouro passa pelo chat.
- Abrir um endereço nunca gasta o convite; só a inscrição o gasta.
- Outro aparelho que apresente o convite enquanto o pedido está pendente é recusado, e o painel alerta.
- Apresentar um convite já encerrado só gera registro. Nunca derruba um Hermes conectado.
- O gesto de colar um bloco pode ser imitado por um golpe: por isso [0008](0008-codigo-digitado.md) e [0017](0017-bloco-sem-script-da-internet.md).
