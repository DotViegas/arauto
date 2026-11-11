# CLAUDE.md

Orientação para sessões de desenvolvimento neste repositório.

## O projeto

O Arauto dá a cada agente Hermes acesso controlado a números de WhatsApp e deixa um Hermes mandar
recado a outro: um **hub** (servidor) e um **conector** (programa ao lado do Hermes). É gratuito, e
o código será aberto, sob a licença Apache-2.0. Na marca, o nome se escreve Arauto[IA]
(`docs/decisoes/0026-nome-do-projeto.md`).

**Estado: primeiro MVP em construção** (`docs/15-mvp.md`). Leia nesta ordem: `README.md`,
`docs/glossario.md`, `docs/01-visao.md`, `docs/04-seguranca-conexao.md`,
`docs/14-protocolo-do-canal.md` e `docs/15-mvp.md`.

## Regras de trabalho

### O que é público e o que é interno

- Tudo o que é versionado aqui é **público**. Não escreva nome de pessoa, de empresa, de servidor,
  endereço interno, telefone ou qualquer dado real. Em exemplos, use endereços e números reservados
  para documentação.
- Anotações de quem mantém o projeto ficam na pasta `interno/`, que está no `.gitignore`. Nunca use
  `git add -f` nela e nunca copie trechos dela para um documento público.
- Antes de qualquer commit, procure por dado real no que vai entrar.

### Vocabulário

- Use os termos de `docs/glossario.md` e respeite a lista "palavras que não usamos".
- **Convite** é o temporário, que morre no primeiro uso. **Credencial lacrada** é o que o Hermes
  guarda para voltar. Não chame nenhum dos dois de "bilhete" ou "token".
- **Conectar** é para Hermes. **Parear** é para número.
- "Sessão" sempre vem com o dono: do canal, do motor, do painel.
- O projeto é gratuito: não escreva sobre preço, venda ou assinatura.

### Fato, proposta e pendência

Todo documento separa:

- **fato**: conferido, com versão e data;
- **proposta**: desenho ainda não provado;
- **decisão pendente**: o que ainda precisa ser escolhido.

Não promova uma proposta a fato sem teste. Não escreva no presente, como se existisse, o que ainda é
desenho. Ao conferir algo no Hermes ou no motor, registre em `referencia/`, com a versão e a data.

### Decisões

- Cada decisão de desenho tem um arquivo em `docs/decisoes/`, com o estado: decidida, proposta ou
  pendente.
- Decisão **decidida** não se reabre sem pedido de quem mantém o projeto.
- Mudou o desenho: atualize o documento afetado **e** a decisão.

### Precisa de aprovação de quem mantém

- O texto do bloco de conexão (`docs/02-como-funciona.md`).
- Qualquer mudança em `docs/04-seguranca-conexao.md`.
- Licença, abertura do repositório e qualquer versão publicada.

## Princípios de segurança

Valem para todo código e todo documento.

1. **Nada de criptografia inventada.** Componha peças conhecidas e diga quais.
2. **Segredo duradouro nunca passa por pessoa:** não aparece em tela, chat, registro, auditoria ou
   resposta de ferramenta. Segredo de curta duração (convite, código de pareamento, código de
   instalação) aparece uma vez, onde é preciso, e nunca é guardado em claro.
3. **O que só se confere guarda-se como chave pública ou resumo.** O que precisa ser lido de volta
   fica cifrado, com a chave fora do banco.
4. **Sorteio só pelo gerador do sistema.** Nada derivado de data, hora, nome ou contador.
5. **Requisição de leitura não muda estado.** Abrir um endereço nunca gasta um convite.
6. **O hub não manda texto livre para a LLM.** Aviso é ponteiro; a frase fixa e as descrições das
   ferramentas ficam no conector.
7. **O conector não executa comando do hub.** A única coisa que ele descarta por informação do hub é
   um pedido de conexão que ainda não recebeu credencial. A pasta de um aparelho antigo sai por
   decisão do conector, depois de outro assumir e de o hub responder.
8. **O hub só isola.** Nada de regra de negócio: sem filtro de conteúdo, sem resposta automática,
   sem escolher qual Hermes age.
9. **Autorização a cada chamada,** no hub, contra o estado atual.
10. **Todo texto vindo de aparelho ou do WhatsApp é hostil:** validar na entrada, escapar na saída.
11. **A sessão do canal só serve depois da troca da credencial confirmada.** Depois da primeira
    confirmada, nunca emitir outra só pela identidade.
12. **Dizer o que não é garantido.** Nunca escrever que o projeto "fecha todas as brechas".

## No código

- Rust nos dois programas. Um espaço de trabalho com cinco partes (`docs/15-mvp.md`, seção 4): o
  protocolo comum, o que muda de um sistema para outro, o hub, o conector e o material de teste.
- Sem `unsafe`. Onde o protocolo (`docs/14-protocolo-do-canal.md`) e o
  `docs/04-seguranca-conexao.md` divergirem, vale o 04.
- O desenvolvimento roda direto pelo `cargo`, com um PostgreSQL local. Container é só para a entrega
  em produção.
- Nomes no código, descrições das ferramentas e mensagens de erro em inglês
  (`docs/decisoes/0016-idioma.md`).
- `Cargo.lock` versionado; compilar com `--locked`.
- Rodar, antes de dizer que algo está pronto: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings` e
  `cargo test --workspace --locked`, com `ARAUTO_TEST_DATABASE_URL` apontando para um PostgreSQL de
  teste. Relatar: o que rodou, o que passou, o que falhou e o que ficou sem cobertura. O que foi só
  lido não conta como testado.
- Regra de segurança nova vem com teste que falha quando a regra é removida.
- Filas e troca de credencial têm teste com rodadas sorteadas: quedas, atrasos e repetições.
- Os testes automáticos rodam contra um motor falso e um Hermes falso. Nada de teste automático num
  número de WhatsApp real. A exceção é o teste de contrato, feito à mão, com um número de teste.

## O que nunca fazer

- Registrar conteúdo de mensagem ou segredo.
- Expor o motor de WhatsApp fora da rede interna.
- Embutir o motor na entrega do projeto.
- Criar um caminho alternativo de conexão "para facilitar" que pule o convite ou o código de
  conferência.
- Abrir porta de rede na máquina do Hermes.
