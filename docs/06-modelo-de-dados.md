# 06 — Modelo de dados

**Estado: proposta.** As tabelas do hub e as regras que valem para todas. As listas de campos trazem
os principais, não todos.

## Resumo

```
organizacao
 ├── admin ── passkey, sessao_painel
 ├── hermes ── aparelho ── conexao_canal
 │      │         └── convite
 │      └── acesso ── acesso_conversa
 ├── motor ── numero ── conversa
 │               └── mensagem ── midia        (arquivo de mensagens)
 ├── aviso                                    (caixa de saída)
 ├── evento_motor                             (caixa de entrada)
 ├── ligacao, fio ── recado                   (conversa entre Hermes)
 └── auditoria, chamada_ferramenta, alerta
instalacao, chave_hub, versao_conector        (da instalação, sem organização)
```

## 1. Regras gerais

| Regra | Detalhe |
|---|---|
| Separação por organização | Toda tabela de negócio pertence a uma organização: tem `organizacao_id`, ou é filha de uma tabela que tem. Toda consulta recebe a organização como parâmetro obrigatório. |
| Identificadores | Sorteados, sem ordem e não adivinháveis. Nunca uma sequência visível. |
| Tempo | UTC em milissegundos, guardado como inteiro, sempre do relógio do hub. Nenhuma coluna usa a hora do banco. |
| Segredo que só se confere | Guarda-se a chave pública ou o resumo SHA-256. Nunca o valor. |
| Segredo que precisa ser lido de volta | Só em coluna terminada em `_cifrado` ou `_cifrada`, com XChaCha20-Poly1305. |
| Chaves | A chave-mestra fica fora do banco. Ela cifra a chave do hub, a chave de cada organização e os segredos do motor. A chave da organização lacra as credenciais e cifra o arquivo de mensagens. |
| Login | Só por passkey. Não há senha. |
| Um banco | PostgreSQL ([decisão 0031](decisoes/0031-banco-postgresql.md)). O que tem prazo de guarda fica em tabelas com expurgo. |
| Acesso ao banco | Só por uma camada de repositório. No banco, os nomes das tabelas são em inglês ([15](15-mvp.md), seção 2). |
| Migrações | Embutidas no programa, só para a frente. Num banco com dados, quem instala tira a cópia de segurança antes e confirma ao aplicar. |

## 2. Quem usa o painel

**organizacao**: `id`, `nome`, `estado` (ativa, suspensa), `chave_cifrada` (a chave da organização,
cifrada pela mestra), `retencao_arquivo_dias`.

**admin**: `id`, `organizacao_id`, `nome`, `email`, `estado`, `criado_em`.

**passkey**: `id`, `admin_id`, `chave_publica`, `contador`, `rotulo`, `criada_em`, `usada_em`.

**sessao_painel**: `id`, `admin_id`, `identificador_sha256`, `expira_em`, `ip`, `encerrada_em`.

Um admin que perde a passkey recebe outra de um segundo admin. Sem segundo admin, um comando no
servidor emite um novo código de instalação.

## 3. Hermes e aparelhos

**hermes**: a linha do painel. `id`, `organizacao_id`, `nome` (único na organização), `modo`
(rápido, blindado), `pode_ser_acordado` e `motivo`, `criado_em`, `excluido_em` (a linha não é
apagada). O nome tem formato restrito, não muda e não é reaproveitado
([13](13-conversa-entre-hermes.md), seção 2).

**aparelho**: uma instalação do conector. `id`, `hermes_id`, `identidade_publica`, `estado`
([10](10-maquinas-de-estado.md)), `vigente_geracao`, `vigente_resumo`, `pendente_geracao`,
`pendente_resumo`, `maior_geracao`, `codigo_de_conferencia` e `confirmar_ate` (apagados ao resolver),
`tentativas`, `inscricoes`, `dados_declarados` (texto não confiável), `ip_inscricao`,
`versao_conector`, `criado_em`, `aprovado_por`, `aprovado_em`, `ultimo_contato_em`, `ultimo_ip`,
`encerrado_em`, `motivo`, `marcado_por_restauracao`.

- No máximo um aparelho em uso e um pedido em andamento por Hermes: a coluna `posicao` (em uso,
  pedido, ou vazia em estado final) tem índice único por Hermes. `primeira_confirmacao_em` diz se o
  aparelho já confirmou credencial.
- O hub não guarda a credencial lacrada: só a geração e o resumo da vigente e da pendente. Eles
  ficam guardados depois de o aparelho ser encerrado: é por eles que o hub reconhece um reuso tardio.
- Um aparelho na geração zero tem `vigente_geracao` igual a zero e nenhum resumo vigente; a
  pendente já pode existir.
- A linha de um aparelho nunca é apagada, nem quando o Hermes é excluído: é por ela que o hub
  reconhece uma identidade encerrada.

**aparelho_desconhecido**: identidade que o banco não conhece, ou que nele era um pedido encerrado
pela restauração, e que apresentou um lacre válido
([04](04-seguranca-conexao.md), C11). `organizacao_id`, `identidade_publica`, `dados_declarados`,
`ip`, `visto_em`.

**convite**: `id`, `hermes_id`, `resumo` (SHA-256 do convite, único; apagado quando vence),
`criado_por`, `expira_em`, `estado`, `usado_em`, `aparelho_id`.

**conexao_canal**: `id`, `aparelho_id`, `ip`, `versao`, `aberta_em`, `troca_confirmada_em`,
`fechada_em`, `motivo`. Uma conexão sem `troca_confirmada_em` nunca chegou a ser sessão do canal.

## 4. Motor, números e conversas

**motor**: `id`, `organizacao_id`, `tipo` (waha), `endereco`, `credencial_cifrada`,
`segredo_de_eventos_cifrado`, `versao_vista`, `ultimo_ok_em`.

**numero**: `id`, `organizacao_id`, `motor_id`, `sessao_no_motor` (nome sorteado pelo hub, nunca
reutilizado), `telefone`, `apelido` (único na organização), `de_comando` (marcação do admin),
`estado`, `estado_desde`, `pareado_em`. O apelido segue o mesmo formato restrito do nome do Hermes.

**conversa**: `id`, `numero_id`, `tipo` (contato, grupo), `identificadores_no_motor` (uma conversa
pode aparecer com mais de um), `nome_exibido` (texto não confiável), `ultima_mensagem_em`.

O Hermes nunca informa o nome da sessão do motor. Ele informa o apelido do número, e o hub resolve a
sessão do motor a partir dos acessos do Hermes daquele aparelho.

## 5. Acessos

**acesso**: um por par (Hermes, número). `id`, `hermes_id`, `numero_id`, `escopo` (todas, lista),
`ler`, `enviar`, `modificar`, `acordar`, `iniciar_conversa`, `criado_por`, `alterado_em`.

**acesso_conversa**: a linha de conversa. `acesso_id`, `conversa_id`, `ler`, `enviar`, `modificar`,
`acordar`. Única por par.

As regras estão em [08-permissoes.md](08-permissoes.md). Toda alteração gera evento de auditoria com
o antes e o depois.

## 6. Filas

**evento_motor** (entrada): `motor_id` + `id_do_evento` (único), `tipo`, `corpo_cifrado`,
`recebido_em`, `processado_em`. O evento é gravado antes de o hub responder ao motor, e só sai depois
de tratado.

**aviso** (saída): `id`, `organizacao_id`, `hermes_id`, `tipo` (mensagem de WhatsApp ou recado),
`numero_id` e `conversa_id` (só no tipo WhatsApp), `remetente_id` (só no tipo recado), `quantidade`,
`estado`, `tentativas`, `nao_antes_de`, `posse_ate`, `expira_em`. Só existe um aviso pendente por
Hermes, tipo e alvo: o que chega depois soma na quantidade.

Esse desenho de filas no banco vem do serviço de origem.

## 7. Arquivo de mensagens

**mensagem**: `id`, `organizacao_id`, `numero_id`, `conversa_id`,
`id_no_motor` (único por número), `direcao`, `autor`, `tipo`, `corpo_cifrado`, `midia_id`,
`hora_no_motor`, `hermes_de_origem` (quando foi um Hermes que enviou), `expurgar_em`.

**midia**: `id`, `organizacao_id`, `sha256`, `tipo`, `tamanho`, `caminho`, `expurgar_em`.

O arquivo de mensagens é o que permite a um Hermes abrir e consultar o WhatsApp dele sem depender de
o motor estar no ar. Retenção e expurgo são obrigatórios: são conversas de terceiros.

## 8. Auditoria

**auditoria**: só acrescenta (o banco proíbe alterar e apagar). `seq`, `organizacao_id`, `quando`,
`ator_tipo` (admin, aparelho, sistema), `ator_id`, `acao` (catálogo fechado), `alvo_tipo`,
`alvo_id`, `ip`, `resultado`, `codigo`, `detalhes`. Na tela, o ator "aparelho" aparece com o nome do
Hermes dele.

A auditoria **nunca** contém conteúdo de mensagem, convite, credencial, QR, código de pareamento ou
chave.

**chamada_ferramenta**: metadado de cada chamada, com retenção curta. `aparelho_id`, `numero_id`,
`ferramenta`, `resultado` (ok, negado, erro), `duracao_ms`, `quando`.

## 9. Da instalação

**instalacao**: `modo_restaurado`, `restaurado_em` (início do modo), `copia_de` (data do último evento
do banco restaurado). A marca da auditoria fica **fora** do banco, ao lado da chave-mestra: é o
número do último evento de segurança gravado. Banco atrás da marca mostra que ele voltou de uma
cópia antiga ([04](04-seguranca-conexao.md), C11).

**chave_hub**: `id`, `chave_publica`, `privada_cifrada` (cifrada pela mestra), `criada_em`, `estado`.

**versao_conector**: `versao`, `sequencia` (número que só cresce; impede voltar a uma versão antiga),
`sha256`, `protocolo_minimo`, `protocolo_maximo`, `assinatura`, `revogada`. Fica para depois do
primeiro MVP.

## 10. Conversa entre Hermes, alertas e retenção

Entram com o primeiro MVP ([15](15-mvp.md)). As regras estão em
[13-conversa-entre-hermes.md](13-conversa-entre-hermes.md).

**ligacao** (a lista de pares): `organizacao_id`, `de_hermes_id`, `para_hermes_id`, `acordar`,
`criada_por`, `alterada_em`. Única por par ordenado. Origem e destino são diferentes e da mesma
organização.

**fio**: `id`, `organizacao_id`, os dois Hermes, `criado_em`, `ultimo_recado_em`.

**recado**: `id`, `organizacao_id`, `fio_id`, `de_hermes_id`, `para_hermes_id`, `enviado_em`,
`tamanho`, `texto_cifrado`, `lido_em`, `gerou_aviso`, `expurgar_em`.

**alerta**: o que o painel mostraria ao admin. `id`, `organizacao_id`, `tipo` (catálogo fechado),
`hermes_id`, `aparelho_id`, `quando`, `detalhes`, `visto_em`.

**retencao**: o prazo de guarda de cada tabela que tem expurgo.
