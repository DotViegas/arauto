# 0022 — Motor sem porta pública, vigiado pelo hub

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

O motor tem a própria API e o próprio painel. Quem os alcança configura um destino extra para os eventos e copia todas as mensagens sem passar por convite, código ou acesso.

## Decisão

O motor fica só na rede interna, com chave própria da instalação. O hub confere a configuração de cada sessão do motor na partida e a cada 5 minutos; se achar um destino de eventos que não é o hub, corrige e alerta.

## Consequências

- O painel do Arauto substitui o painel do motor.
- O volume de sessões do motor fica fora da cópia de segurança comum: quem o tem, tem o WhatsApp.
- Parear, reiniciar e remover números só pelo painel, por um admin.
