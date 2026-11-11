# 0032 — Conversa entre Hermes no primeiro MVP: recados com lista

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

A conversa entre Hermes estava na visão, sem desenho e para depois da primeira versão. Quem mantém o projeto a definiu como a função principal do Arauto.

## Decisão

A conversa entre Hermes entra no primeiro MVP, no formato de **recados com lista**: um Hermes manda um recado a outro pelo hub; o recado só passa se o par estiver na lista que o admin marca; quem recebe é avisado e decide o que fazer. O recado é dado, nunca ordem. Há tetos de quantidade, e tudo fica registrado. A especificação está em [13-conversa-entre-hermes.md](../13-conversa-entre-hermes.md).

## Consequências

- Vale a separação entre comando e dado ([0005](0005-comando-e-dado-sao-separados.md)): um Hermes não manda em outro pelo Arauto. A alternativa, recado com autoridade de ordem, foi descartada.
- O hub continua só isolando ([0004](0004-o-hub-so-isola.md)): não lê o conteúdo, não escolhe quem responde. Os tetos entram como isolamento, como o teto de envio ([0020](0020-teto-de-envio.md)).
- Salas com vários Hermes e o observador de um par ficam para depois.
- Os tetos são contados por ligação e por destinatário, nunca por fio: abrir um fio novo não zera nenhum. O valor é fixo na primeira versão e é calibrado no MVP.
- Não há prova de remetente de ponta a ponta: quem controla o hub escreve como qualquer Hermes. Fica dito como limite.
- O aviso de recado segue a regra do ponteiro ([0018](0018-aviso-e-ponteiro.md)). A redação da frase fixa depende de aprovação.
