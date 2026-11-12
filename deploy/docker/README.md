# Arauto com Docker Compose

O hub e o banco numa composição. O proxy e o motor de WhatsApp são seus e ficam fora dela.

| Arquivo | O que é |
|---|---|
| [`Dockerfile`](Dockerfile) | A imagem do hub. Traz também o conector para Linux x86_64, servido para download. |
| [`compose.yaml`](compose.yaml) | O hub e o PostgreSQL 16, com os volumes e as redes. |
| [`arauto.env.example`](arauto.env.example) | As variáveis que você preenche. |

## O que falta provar

Estes arquivos foram escritos junto com o código, mas **a imagem ainda não foi construída e a
composição ainda não subiu num servidor**: a máquina de desenvolvimento do projeto não tem Docker.
O primeiro uso num servidor é o teste. O que conferir nele está no fim desta página.

## 1. Preparar

Você precisa de um servidor Linux x86_64 com Docker e o plugin Compose, de um nome de domínio
apontando para ele, e de um proxy com TLS ([exemplos](../proxy/README.md)).

```
git clone <o repositório> arauto
cd arauto/deploy/docker
cp arauto.env.example arauto.env
```

Preencha o `arauto.env`:

| Variável | O que pôr |
|---|---|
| `POSTGRES_PASSWORD` | Uma senha longa e sorteada. Por exemplo: `openssl rand -hex 32`. |
| `ARAUTO_PUBLIC_URL` | O endereço de fora, com `https://`. Só o nome: sem caminho. |
| `ARAUTO_TRUSTED_PROXIES` | O endereço do seu proxy como o hub o enxerga, ou a rede dele. |

O `arauto.env` guarda uma senha. Não o ponha num repositório.

## 2. Construir e iniciar

```
docker compose --env-file arauto.env build
docker compose --env-file arauto.env up -d postgres
docker compose --env-file arauto.env run --rm hub init
docker compose --env-file arauto.env up -d
docker compose --env-file arauto.env ps
```

- `init` roda **uma vez**. Ele cria a chave-mestra, a marca da auditoria, a organização e a chave do
  hub. Rodar de novo não refaz nada.
- Depois do `up`, o serviço `hub` precisa aparecer como `healthy`. A conferência de saúde é o
  comando `arauto-hub health`, que pergunta pela porta interna.

Daqui em diante, todo comando de administração roda dentro do container:

```
docker compose --env-file arauto.env exec hub arauto-hub admin status
```

## 3. Conectar o primeiro Hermes

```
docker compose --env-file arauto.env exec hub arauto-hub admin hermes add atendimento
docker compose --env-file arauto.env exec hub arauto-hub admin invite atendimento
```

O `invite` mostra o bloco: o endereço do hub, a chave do hub, o convite (vale 15 minutos e um uso),
o endereço para baixar o conector e o SHA-256 do arquivo. Na máquina do Hermes: baixe o conector,
**confira o SHA-256** e só então rode o `arauto-connector join ...` do bloco. O Hermes responde um
código de seis dígitos. Você o digita aqui:

```
docker compose --env-file arauto.env exec hub arauto-hub admin approve atendimento 123456
```

O Hermes nasce conectado e sem acesso a nada.

## 4. Ligar o motor de WhatsApp

O motor é seu. Ponha-o na rede `arauto-engine` (o nome está em `ARAUTO_ENGINE_NETWORK`), **sem porta
pública**. Nessa rede o hub alcança o motor, e o motor alcança o hub em `http://hub:8444`.

```
docker compose --env-file arauto.env exec hub arauto-hub admin engine set --url http://waha:3000 --events-url http://hub:8444
docker compose --env-file arauto.env exec -it hub arauto-hub admin engine set-key
docker compose --env-file arauto.env exec hub arauto-hub admin engine health
```

O `set-key` pede a chave da API do motor no terminal, para ela não ficar no histórico.

Depois, o número e os acessos:

```
... admin number add balcao +5565900000000
... admin number pair balcao
... admin wake atendimento on
... admin access grant atendimento balcao --scope all --read --send --wake
```

## 5. Deixar dois Hermes conversarem

```
... admin link add atendimento financeiro --wake --both
```

## Cópia de segurança

- **Do banco:** o volume `arauto-postgres`, por `pg_dump` ou por arquivamento do registro de
  transações. Diária, no mínimo.
- **Da pasta de dados** (volume `arauto-data`): guarde **separada** da cópia do banco, num lugar
  mais protegido. Ela tem a chave-mestra. Sem ela, o banco restaurado não abre nenhuma credencial.
- Restaurar o banco de uma cópia liga o modo restaurado: os Hermes param de ser atendidos até
  reconectarem, cada um com um convite novo. É de propósito: uma cópia antiga traz credenciais
  antigas.

## Atualizar

```
git pull
docker compose --env-file arauto.env build
docker compose --env-file arauto.env run --rm hub migrate --i-have-a-backup
docker compose --env-file arauto.env up -d
```

Faça a cópia de segurança do banco antes do `migrate`: o nome do argumento é o lembrete.

## O que conferir no primeiro uso num servidor

| Conferência | Como |
|---|---|
| A imagem constrói | `docker compose build` termina sem erro. |
| O hub passa na saúde | `docker compose ps` mostra `healthy`. |
| O banco força a gravação | O hub sobe sem pedir `ARAUTO_ALLOW_UNSAFE_DB`. |
| A porta interna não é publicada | De fora do servidor, a porta 8444 não responde. |
| O download funciona | `curl -O https://SEU-ENDERECO/v1/download/arauto-connector-linux-x86_64` e o `sha256sum` bate com o do bloco. |
| O endereço dos conectores | Em `admin audit`, a conexão de um Hermes mostra o endereço dele, não o do proxy. |
| A parada em ordem | `docker compose stop hub` termina em menos de 20 segundos, sem ser forçada. |
