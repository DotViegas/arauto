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
| `ARAUTO_TRUSTED_PROXIES` | O endereço do seu proxy **como o hub o enxerga**. Veja abaixo. |

O `arauto.env` guarda uma senha. Não o ponha num repositório.

**O endereço do proxy, visto pelo hub.** Com o proxy no próprio servidor apontando para
`127.0.0.1:8443`, o hub **não** enxerga `127.0.0.1` nem o endereço do servidor: ele enxerga o
gateway de uma rede do Docker. A composição fixa essas redes, e o exemplo já traz os dois gateways
(`172.30.250.1` e `172.30.251.1`). Se você trocar as sub-redes, troque a lista junto.

Lista errada não impede o hub de subir. Ele ignora o cabeçalho, trata todos os conectores como se
viessem de um endereço só, e avisa **uma vez** no registro: "a connection that is not from a trusted
proxy brought X-Forwarded-For". Se essa linha aparecer, o endereço que ela mostra é o que falta na
lista.

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

**O registro do hub** sai em `docker compose --env-file arauto.env logs hub`. Ao subir ele diz
"the hub is up", com os dois endereços (três, com o painel ligado). É ali que aparecem os avisos do motor de WhatsApp, a lista
de proxies errada e o modo restaurado. Para mais detalhe, ponha `RUST_LOG=debug` no `arauto.env`.

## Abrir o painel (opcional)

O painel mostra os Hermes, os números e os acessos, e conecta um Hermes, passo a passo. Parear
números e dar acesso continuam pelo comando. Vem desligado, e **ainda não foi provado num
servidor**.

Ainda não há passkey: quem está com a sessão do painel aberta pode conectar e reconectar Hermes.
Por isso, deixe o painel fechado, como abaixo, e saia dele ao terminar.

1. No `arauto.env`, tire o comentário de `ARAUTO_LISTEN_PANEL` e de `ARAUTO_PANEL_URL`, e rode
   `docker compose --env-file arauto.env up -d`.
2. Do seu computador, abra um túnel até o servidor: `ssh -L 8445:127.0.0.1:8445 SEU-SERVIDOR`.
3. Emita o código de entrada: `docker compose --env-file arauto.env exec hub arauto-hub admin panel code`.
   Ele aparece uma vez, vale 10 minutos e uma entrada. É segredo: não o cole em conversa nem em
   relatório.
4. Abra `http://localhost:8445` no navegador e digite o código.

O endereço no navegador tem de ser exatamente o do `ARAUTO_PANEL_URL`: com outro nome, o hub
responde 421. A sessão do painel dura 12 horas, ou 30 minutos sem uso, e acaba quando o hub
reinicia.

A porta do painel é publicada só em `127.0.0.1` do servidor. Dentro do Docker, quem está numa rede
do hub (o motor de WhatsApp, por exemplo) alcança a tela de entrada; sem o código, não passa dela.
Abrir o painel sem túnel (um endereço próprio no seu proxy, com TLS, que não seja o dos
conectores) é possível, mas **não é recomendado enquanto a passkey não existir**. O risco de abri-lo
é seu.

## 3. Conectar o primeiro Hermes

```
docker compose --env-file arauto.env exec hub arauto-hub admin hermes add atendimento
docker compose --env-file arauto.env exec hub arauto-hub admin invite atendimento
```

O `invite` mostra o bloco: o endereço do hub, a chave do hub, o convite (vale 15 minutos e um uso),
o endereço do conector, o SHA-256 do arquivo e **um comando só**, encadeado: ele baixa o conector
para `~/.arauto`, confere o SHA-256 escrito no bloco e só então roda o `join`. Se o arquivo não
confere, nada roda.

Esse comando roda **na máquina do Hermes, com o mesmo usuário do Hermes** (de preferência pelo
próprio Hermes, colando o bloco no canal de comando dele). Não o rode como outro usuário nem por
`docker exec` como root: o `join` registra o conector no Hermes chamando o programa `hermes`, e
precisa encontrá-lo.

O `join` responde um código de seis dígitos, no formato `182-239`. Você o digita aqui, com ou sem
o hífen:

```
docker compose --env-file arauto.env exec hub arauto-hub admin approve atendimento 182-239
```

O Hermes nasce conectado e sem acesso a nada.

| Se o `join` disser | O que fazer |
|---|---|
| `AR-601` (o registro no Hermes falhou) | O programa `hermes` não foi achado, ou recusou. Diga onde ele está com a variável `ARAUTO_HERMES_BIN` e repita o mesmo `join`; ou repita com `--no-register` e registre à mão. O pedido continua aberto no hub. |
| `AR-101` ou `AR-102` | O convite venceu ou já foi usado. Emita outro com `admin invite`. |

Um **reconectar** (outro convite para um Hermes que já tem aparelho) não registra de novo no
Hermes: a entrada que já existe continua valendo.

## 4. Ligar o motor de WhatsApp

O motor é seu. Ponha-o na rede `arauto-engine` (o nome está em `ARAUTO_ENGINE_NETWORK`), **sem porta
pública**. Nessa rede o hub alcança o motor, e o motor alcança o hub em `http://hub:8444`.

```
docker compose --env-file arauto.env exec hub arauto-hub admin engine set --url http://waha:3000 --events-url http://hub:8444
docker compose --env-file arauto.env exec -it hub arauto-hub admin engine set-key
docker compose --env-file arauto.env exec hub arauto-hub admin engine health
```

O `set-key` pede a chave da API do motor no terminal, para ela não ficar no histórico.

**Confira os dois sentidos.** O `engine health` só prova que o hub alcança o motor. O caminho de
volta, do motor para o hub, é por onde as mensagens chegam na hora:

```
docker run --rm --network arauto-engine curlimages/curl -s http://hub:8444/health
```

Tem de responder `ok`. Se esse caminho estiver errado, nada acusa: o hub ainda busca as mensagens
sozinho a cada 5 minutos, e tudo parece funcionar, só que atrasado. Depois de parear, mande uma
mensagem ao número: ela tem de aparecer em segundos, não em minutos.

Depois, o número e os acessos:

```
... admin number add balcao +5565900000000
... admin number pair balcao
... admin wake atendimento on
... admin access grant atendimento balcao --scope all --read --send --wake
```

**Para o Hermes ser acordado** falta uma coisa que nenhum comando do hub faz: o conector precisa da
chave da API local do Hermes. Ela chega por duas variáveis na entrada `arauto` do servidor de
ferramentas do Hermes: `ARAUTO_HERMES_API_KEY` (a chave) e `ARAUTO_HERMES_API_URL` (o endereço, com o
número da máquina e a porta, como `http://127.0.0.1:8642/v1/runs`; `localhost` não é aceito). Sem
a chave, as ferramentas funcionam e ninguém é acordado. Confira na máquina do Hermes:

```
~/.arauto/arauto-connector-linux-x86_64 status
```

Ele mostra duas linhas: `Waking allowed by the admin` (o que você liberou no hub) e `Key of the
local API of Hermes` (se o conector tem a chave). O Hermes só acorda com as duas em ordem. O formato
exato dessa chamada ainda não foi provado contra um Hermes de verdade: é uma das conferências do
primeiro uso.

Um acesso ao número inteiro acorda em conversa com uma pessoa. Um **grupo** só acorda o Hermes que
tem uma linha própria para aquele grupo (`admin access chat ... --read --wake`). E o aviso espera a
conversa ficar quieta por 6 segundos antes de acordar; mude com `admin access wake-wait`.

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
| A porta do painel não é publicada | De fora do servidor, a porta 8445 não responde, com o painel ligado ou desligado. |
| O painel não existe no endereço dos conectores | `curl -I https://SEU-ENDERECO/sign-in` responde 404. |
| O download funciona | `curl -O https://SEU-ENDERECO/v1/download/arauto-connector-linux-x86_64` e o `sha256sum` bate com o do bloco. |
| O endereço dos conectores | Em `admin audit`, a conexão de um Hermes mostra o endereço dele, não o do proxy. |
| A parada em ordem | `docker compose stop hub` termina em menos de 20 segundos, sem ser forçada. |
