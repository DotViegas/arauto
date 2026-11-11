# 0035 — Pilha do websocket e formato das mensagens

**Estado:** proposta · **Data:** 10/11/2026 · revisada no mesmo dia, depois da revisão do desenho

## Contexto

O canal é um websocket com Noise por dentro ([0012](0012-canal-cifrado-por-dentro.md)). Faltava escolher as peças e o formato do que passa nele, com o desempenho em vista.

## Decisão

- Servidor: `axum` sobre `hyper`, no `tokio` com vários núcleos. O laço que aceita conexões é do projeto, para ter prazo de cabeçalhos, teto de conexões e cancelamento por conexão. Websocket só com mensagens binárias, de até 65.535 bytes, e sem compressão.
- Cada conexão tem uma tarefa que lê e outra que escreve. Duas filas de saída, contadas em quadros: controle e dados. Quem produz dado espera lugar; a conexão só é fechada quando a escrita para de andar ou a fila de controle enche.
- O Noise é usado no modo em que o contador de cada sentido é passado à mão. Só a tarefa que escreve cifra, depois de escolher a fila. A recusa de quadro repetido ou fora de ordem é código do projeto, com teste.
- Mensagens em JSON, dentro do Noise, cortadas em pedaços de até 60.000 bytes com número de fluxo, com limites por fase ([14-protocolo-do-canal.md](../14-protocolo-do-canal.md)).
- Banco pelo `sqlx`, com um conjunto fixo de conexões. Autorização a cada chamada por consulta com índice, sem memória intermediária.
- Entre processos, o comando de administração avisa o hub por `NOTIFY`. Como esse aviso se perde, o hub também varre a cada 2 segundos.

## Consequências

- JSON custa mais processador e mais bytes do que um formato binário. Foi escolhido porque as chamadas e respostas das ferramentas já são JSON, porque tipo desconhecido pode ser ignorado e porque o custo que pesa é o da cifra e o do banco. Se o teste de carga mostrar o contrário, troca-se o formato numa versão nova do protocolo.
- Sem memória intermediária de acessos, tirar um acesso vale na chamada seguinte sem depender de invalidação. O custo é uma consulta por chamada.
- A meta de alguns milhares de conectores por hub só vira dado quando o teste de carga do repositório rodar.
- Alternativas consideradas: uma biblioteca de websocket mais enxuta, que ganha na leitura dos quadros e não no que pesa aqui; e mensagens em formato binário, pelos motivos acima.
