# 0031 — Banco: PostgreSQL

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

O desenho anterior usava SQLite, em dois arquivos: um pequeno e crítico, outro com as mensagens. A escolha vinha do serviço de origem. O modelo de dados já previa a troca: todo acesso ao banco passa por uma camada de repositório.

## Decisão

O hub usa PostgreSQL. Em produção ele sobe em container, ao lado do hub, pelo arquivo de composição. Em desenvolvimento, usa-se um PostgreSQL local, sem container.

## Consequências

- Um banco só. A separação em dois arquivos existia porque o SQLite tem um escritor; no PostgreSQL, as mensagens ficam em tabelas com expurgo por data ([15-mvp.md](../15-mvp.md), seção 2).
- As filas continuam no banco, com `FOR UPDATE SKIP LOCKED`. O hub é avisado por `LISTEN/NOTIFY` e também confere de tempos em tempos.
- A chave-mestra e a marca da auditoria continuam fora do banco e fora da cópia de segurança dele ([04](../04-seguranca-conexao.md), C11).
- A cópia de segurança passa a ser a do PostgreSQL. A regra de [03-arquitetura.md](../03-arquitetura.md), seção 8, é reescrita com isso.
- O banco é mais um serviço para quem instala cuidar. Em troca, mais de um processo escreve ao mesmo tempo, o que o comando de administração e, mais adiante, um segundo hub precisam.
- Quem só lê o banco continua sem conseguir se passar por um Hermes: nada do que se guarda mudou.
