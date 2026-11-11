# 0017 — Bloco sem script da internet

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

O jeito mais curto de instalar algo é mandar o conteúdo de um endereço direto para o terminal. É também o gesto de um golpe: quem cola não vê o que roda, e quem controla o endereço escolhe o que roda a cada pedido.

## Decisão

O bloco baixa o conector para um arquivo, numa versão exata, confere o resumo SHA-256 escrito no próprio bloco e só então executa. O convite vai como argumento do conector, nunca num endereço.

## Consequências

- Quem cola vê de onde o programa vem e qual versão exata. O resumo fixa essa versão, igual para todos.
- O resumo vem do painel autenticado e é o mesmo publicado nas versões do projeto, para conferência por fora.
- O convite não aparece em registro de servidor nem em pré-visualização.
- Toda a lógica de instalação fica no conector. O que a LLM vê no chat não ensina a registrar servidores de ferramentas.
- Continua sendo um programa da internet rodando no Hermes. O que muda é que a versão é fixa e conferível.
