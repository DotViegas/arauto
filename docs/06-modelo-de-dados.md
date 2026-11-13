# 06 — Data model

**Status: proposal.** The tables of the hub and the rules that apply to all of them. The field lists
bring the main ones, not all.

## Summary

```
organizacao
 ├── admin ── passkey, sessao_painel
 ├── hermes ── aparelho ── conexao_canal
 │      │         └── convite
 │      └── acesso ── acesso_conversa
 ├── motor ── numero ── conversa
 │               └── mensagem ── midia        (message archive)
 ├── aviso                                    (outbox)
 ├── evento_motor                             (inbox)
 ├── ligacao, fio ── recado                   (conversation between Hermes)
 └── auditoria, chamada_ferramenta, alerta
instalacao, chave_hub, versao_conector        (of the installation, no organization)
```

## 1. General rules

| Rule | Detail |
|---|---|
| Separation by organization | Every business table belongs to an organization: it has `organizacao_id`, or is a child of a table that has it. Every query receives the organization as a mandatory parameter. |
| Identifiers | Randomly drawn, unordered and not guessable. Never a visible sequence. |
| Time | UTC in milliseconds, stored as an integer, always from the hub's clock. No column uses the database's time. |
| Secret that is only checked | The public key or the SHA-256 digest is kept. Never the value. |
| Secret that has to be read back | Only in a column ending in `_cifrado` or `_cifrada`, with XChaCha20-Poly1305. |
| Keys | The master key stays outside the database. It encrypts the hub key, the key of each organization and the secrets of the engine. The organization's key seals the credentials and encrypts the message archive. |
| Login | Only by passkey. There is no password. |
| One database | PostgreSQL ([decision 0031](decisoes/0031-banco-postgresql.md)). What has a retention period stays in tables with purge. |
| Access to the database | Only through a repository layer. In the database, the table names are in English ([15](15-mvp.md), section 2). |
| Migrations | Embedded in the program, forward only. On a database with data, whoever installs takes the backup first and confirms when applying. |

## 2. Who uses the panel

**organizacao**: `id`, `nome`, `estado` (active, suspended), `chave_cifrada` (the organization's key,
encrypted by the master key), `retencao_arquivo_dias`.

**admin**: `id`, `organizacao_id`, `nome`, `email`, `estado`, `criado_em`.

**passkey**: `id`, `admin_id`, `chave_publica`, `contador`, `rotulo`, `criada_em`, `usada_em`.

**sessao_painel**: `id`, `admin_id`, `identificador_sha256`, `expira_em`, `ip`, `encerrada_em`.

An admin who loses the passkey receives another from a second admin. With no second admin, a command
on the server issues a new installation code.

## 3. Hermes and devices

**hermes**: the line of the panel. `id`, `organizacao_id`, `nome` (unique in the organization), `modo`
(quick, shielded), `pode_ser_acordado` and `motivo`, `criado_em`, `excluido_em` (the line is not
erased). The name has a restricted format, does not change and is not reused
([13](13-conversa-entre-hermes.md), section 2).

**aparelho**: an installation of the connector. `id`, `hermes_id`, `identidade_publica`, `estado`
([10](10-maquinas-de-estado.md)), `vigente_geracao`, `vigente_resumo`, `pendente_geracao`,
`pendente_resumo`, `maior_geracao`, `codigo_de_conferencia` and `confirmar_ate` (erased on resolving),
`tentativas`, `inscricoes`, `dados_declarados` (untrusted text), `ip_inscricao`,
`versao_conector`, `criado_em`, `aprovado_por`, `aprovado_em`, `ultimo_contato_em`, `ultimo_ip`,
`encerrado_em`, `motivo`, `marcado_por_restauracao`.

- At most one device in use and one request in progress per Hermes: the column `posicao` (in use,
  request, or empty in a final state) has a unique index per Hermes. `primeira_confirmacao_em` says
  whether the device has already confirmed a credential.
- The hub does not keep the sealed credential: only the generation and the digest of the current one
  and of the pending one. They stay stored after the device is closed: it is by them that the hub
  recognizes a late reuse.
- A device in generation zero has `vigente_geracao` equal to zero and no current digest; the
  pending one may already exist.
- The line of a device is never erased, not even when the Hermes is deleted: it is by it that the hub
  recognizes a closed identity.

**aparelho_desconhecido**: an identity the database does not know, or that in it was a request closed
by the restoration, and that presented a valid seal
([04](04-seguranca-conexao.md), C11). `organizacao_id`, `identidade_publica`, `dados_declarados`,
`ip`, `visto_em`, `prova_pendente`. The pending proof says that this listing is still a proof of
restoration to be handled: the line is written before restored mode is turned on, and turning the
mode on clears all the pending ones in the same write ([14](14-protocolo-do-canal.md), section 6).

**convite**: `id`, `hermes_id`, `resumo` (SHA-256 of the invite, unique; erased when it expires and,
in the hub, also when it is cancelled), `criado_por`, `expira_em`, `estado`, `usado_em`, `aparelho_id`.

**conexao_canal**: `id`, `aparelho_id`, `ip`, `versao`, `aberta_em`, `troca_confirmada_em`,
`fechada_em`, `motivo`. A connection without `troca_confirmada_em` never got to be a channel session.
In the hub, it also keeps when it received a credential: it is the count for the alert of swaps
without "saved". The reason comes from a closed catalog: drop, silence, protocol error, attempt
replaced by another, taken for dead, "saved" past the deadline, "saved" refused, device closed,
presentation without swap, hub stopping and hub stopped. On start, the hub closes the ones the
previous run left open.

Every line that points to a device belongs to its organization: the database refuses a connection or
an invite of one organization that points to a device of another.

## 4. Engine, numbers and chats

**motor**: `id`, `organizacao_id`, `tipo` (waha), `endereco`, `credencial_cifrada`,
`segredo_de_eventos_cifrado`, `versao_vista`, `ultimo_ok_em`.

**numero**: `id`, `organizacao_id`, `motor_id`, `sessao_no_motor` (name randomly drawn by the hub,
never reused), `telefone`, `apelido` (unique in the organization), `de_comando` (marking by the
admin), `estado`, `estado_desde`, `pareado_em`. The alias follows the same restricted format as the
name of the Hermes.

**conversa**: `id`, `numero_id`, `tipo` (contact, group), `identificadores_no_motor` (a chat
may appear with more than one), `nome_exibido` (untrusted text), `ultima_mensagem_em`.

Hermes never states the name of the engine session. It states the alias of the number, and the hub
resolves the engine session from the accesses of the Hermes of that device.

## 5. Accesses

**acesso**: one per pair (Hermes, number). `id`, `hermes_id`, `numero_id`, `escopo` (all, list),
`ler`, `enviar`, `modificar`, `acordar`, `iniciar_conversa`, `criado_por`, `alterado_em`.

**acesso_conversa**: the chat line. `acesso_id`, `conversa_id`, `ler`, `enviar`, `modificar`,
`acordar`. Unique per pair.

The rules are in [08-permissoes.md](08-permissoes.md). Every change produces an audit event with
the before and the after.

In the hub, on 12/11/2026 (proposal, [decision 0039](decisoes/0039-matriz-e-ferramentas-de-whatsapp-no-mvp.md)):
the tables are `access` (`scope`, `can_read`, `can_send`, `can_modify`, `can_wake`,
`can_start_chats`, `created_by`, `created_at`, `changed_at`) and `access_chat` (with `number_id`, so
that the database refuses a chat line of another number). The database also refuses "wake" without
"read" and "start chats" without "send" and scope "all". Removing the number or the Hermes erases
its accesses, each one with its event.

## 6. Queues

**evento_motor** (inbound): `motor_id` + `id_do_evento` (unique), `tipo`, `corpo_cifrado`,
`recebido_em`, `processado_em`. The event is written before the hub answers the engine, and only
leaves after it is handled.

**aviso** (outbound): `id`, `organizacao_id`, `hermes_id`, `tipo` (WhatsApp message or note),
`numero_id` and `conversa_id` (only in the WhatsApp type), `remetente_id` (only in the note type),
`quantidade`, `estado`, `tentativas`, `nao_antes_de`, `posse_ate`, `expira_em`. There is only one
pending notice per Hermes, type and target: what arrives later adds to the quantity.

This design of queues in the database comes from the source service.

## 7. Message archive

**mensagem**: `id`, `organizacao_id`, `numero_id`, `conversa_id`,
`id_no_motor` (unique per number), `direcao`, `autor`, `tipo`, `corpo_cifrado`, `midia_id`,
`hora_no_motor`, `hermes_de_origem` (when it was a Hermes that sent it), `expurgar_em`.

**midia**: `id`, `organizacao_id`, `sha256`, `tipo`, `tamanho`, `caminho`, `expurgar_em`.

The message archive is what allows a Hermes to open and query its WhatsApp without depending on
the engine being up. Retention and purge are mandatory: they are chats of third parties.

## 8. Audit

**auditoria**: append only (the database forbids changing and erasing). `seq`, `organizacao_id`,
`quando`, `ator_tipo` (admin, device, system), `ator_id`, `acao` (closed catalog), `alvo_tipo`,
`alvo_id`, `ip`, `resultado`, `codigo`, `seguranca`, `detalhes`. On the screen, the actor "device"
appears with the name of its Hermes. `seguranca` says whether the event changes who may do what: only
those advance the mark ([15](15-mvp.md), section 2, rule 10). The events of the installation and the
refusals of invites that nobody issued have no organization.

The audit **never** contains message content, invite, credential, QR, pairing code or
key.

**chamada_ferramenta**: metadata of each call, with short retention. `aparelho_id`, `numero_id`,
`ferramenta`, `resultado` (ok, denied, error), `duracao_ms`, `quando`.

## 9. Of the installation

**instalacao**: `modo_restaurado`, `restaurado_em` (start of the mode), `copia_de` (date of the last
event of the restored database). The audit mark stays **outside** the database, beside the master
key: it is the number of the last security event written. A database behind the mark shows that it
came back from an old backup ([04](04-seguranca-conexao.md), C11).

**chave_hub**: `id`, `chave_publica`, `privada_cifrada` (encrypted by the master key), `criada_em`,
`estado`.

**versao_conector**: `versao`, `sequencia` (a number that only grows; it prevents going back to an
old version), `sha256`, `protocolo_minimo`, `protocolo_maximo`, `assinatura`, `revogada`. It is left
for after the first MVP.

## 10. Conversation between Hermes, alerts and retention

They come in with the first MVP ([15](15-mvp.md)). The rules are in
[13-conversa-entre-hermes.md](13-conversa-entre-hermes.md).

**ligacao** (the peer list): `organizacao_id`, `de_hermes_id`, `para_hermes_id`, `acordar`,
`criada_por`, `alterada_em`. Unique per ordered pair. Origin and destination are different and of the
same organization.

**fio**: `id`, `organizacao_id`, the two Hermes, `criado_em`, `ultimo_recado_em`.

**recado**: `id`, `organizacao_id`, `fio_id`, `de_hermes_id`, `para_hermes_id`, `enviado_em`,
`tamanho`, `texto_cifrado`, `lido_em`, `gerou_aviso`, `expurgar_em`.

**alerta**: what the panel would show the admin. `id`, `organizacao_id`, `tipo` (closed catalog),
`hermes_id`, `aparelho_id`, `quando`, `detalhes`, `visto_em`.

The alert types in the hub, on 11/11/2026:

| Type | When |
|---|---|
| `invite_misused` | The invite was used by another identity (C3). One per request. |
| `identity_copied` | Two waiting connections with the same identity (C6). |
| `quarantined` | A device went to quarantine. |
| `copy_suspected` | The admin did not recognize the last use of the old device, when approving a reconnect (C10). |
| `connection_taken` | A session that was serving was taken for dead and another connection came in its place (C9). |
| `device_left` | The device itself asked for the closing (C10). |
| `reuse_after_closing` | An older seal presented by a closed device (C7). At most one per device every 24 hours, like the log entry of a seal with no explanation. |
| `unconfirmed_swaps` | Three connections without "saved" in 24 hours (C7). |
| `mark_not_advanced` | A security event was written, but the audit mark did not follow. Without it, a restoration that lost exactly that event would go unnoticed. |
| `engine_version_untested` | The engine version changed to one that is not in the list of tested ones ([waha-fatos](../referencia/waha-fatos.md)). One per version change. |
| `engine_session_corrected` | The supervision found an engine session with a configuration different from the hub's (for example, another destination for the events) and corrected it ([10](10-maquinas-de-estado.md), section 4). |
| `number_swapped` | The phone that paired a number is not the telephone the admin added: the accesses stay stopped until `admin number confirm` or `unpair` ([10](10-maquinas-de-estado.md), section 4). |

**retencao**: the retention period of each table that has purge.
