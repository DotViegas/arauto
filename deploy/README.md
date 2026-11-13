# Como instalar o Arauto

Cada pasta daqui é uma forma de instalar o hub, com os arquivos e um guia próprio. O que vale para
todas está nesta página.

| Pasta | Para quem | Estado |
|---|---|---|
| [`docker/`](docker/README.md) | Um servidor com Docker e Docker Compose. É o caminho recomendado. | Escrito. **Ainda não construído nem testado num servidor**: veja "O que falta provar", no guia. |
| [`proxy/`](proxy/README.md) | Quem precisa de um exemplo de proxy na frente do hub (Caddy ou NGINX). | Exemplos escritos, não testados. |
| [`kubernetes/`](kubernetes/README.md) | Quem roda em Kubernetes. | Só a lista do que os manifestos precisam garantir. Os manifestos não foram escritos. |

O conector não se instala por aqui: ele é baixado do próprio hub, pelo bloco que o comando
`admin invite` mostra ([docs/02-como-funciona.md](../docs/02-como-funciona.md)).

## As peças

```
                 internet                         rede interna do servidor
 CONECTOR ──wss──► PROXY ──http──► HUB :8443 ─────► POSTGRESQL
 (ao lado de                (TLS)   │
  cada Hermes)                      │ :8444 (interna, nunca publicada)
                                    ◄──── eventos ──── MOTOR DE WHATSAPP
                                    ────► chamadas ───► (de quem instala)
```

| Peça | De quem é | O que precisa |
|---|---|---|
| **Hub** | Do projeto: a imagem. | Duas portas: a dos conectores (publicada atrás do proxy) e a interna (saúde e eventos do motor). |
| **PostgreSQL 16** | Vem na composição. | Gravação forçada em disco ligada (`fsync`). O hub se recusa a subir sem ela. |
| **Proxy** | De quem instala. | Terminar o TLS, repassar a troca para websocket, prazo de inatividade acima de 90 segundos. |
| **Motor de WhatsApp** | De quem instala. O projeto não o embute. | Estar numa rede que alcança a porta interna do hub, e nunca ter porta pública. |

## O que toda instalação precisa garantir

1. **Dois volumes separados.** O banco num, a pasta de dados do hub (a chave-mestra e a marca da
   auditoria) noutro. A cópia de segurança do banco **não** leva a pasta de dados: quem rouba a
   cópia do banco não leva a chave.
2. **Só a porta dos conectores é publicada**, e atrás de um proxy com TLS. A porta interna e o motor
   nunca têm porta pública.
3. **O endereço de cada conector.** Atrás de um proxy, o hub lê o endereço do cabeçalho
   `X-Forwarded-For`, e só acredita nele quando a conexão vem de um proxy da lista
   `ARAUTO_TRUSTED_PROXIES`. Lista errada deixa qualquer um forjar o próprio endereço.
4. **Cópia de segurança do banco**, contínua ou, no mínimo, diária. Restaurar uma cópia liga o modo
   restaurado: todos os Hermes precisam reconectar
   ([docs/04-seguranca-conexao.md](../docs/04-seguranca-conexao.md), C11).
5. **Limite de arquivos abertos** alto para o hub: cada conector mantém uma conexão.
6. **Prazo de parada** de pelo menos 20 segundos: o hub deixa as chamadas em andamento responderem
   antes de sair.

## As variáveis do hub

| Variável | Para quê |
|---|---|
| `ARAUTO_DATABASE_URL` | O banco PostgreSQL. Leva a senha: nunca aparece em registro. |
| `ARAUTO_DATA_DIR` | A pasta da chave-mestra e da marca da auditoria. |
| `ARAUTO_LISTEN` | O endereço da porta dos conectores. |
| `ARAUTO_LISTEN_INTERNAL` | O endereço da porta interna. |
| `ARAUTO_PUBLIC_URL` | O endereço que vai no bloco: `https://` e só o nome (e a porta). |
| `ARAUTO_CLIENT_IP_SOURCE` | De onde vem o endereço de um conector: `socket` ou `forwarded`. |
| `ARAUTO_TRUSTED_PROXIES` | Com `forwarded`: os proxies em que o hub acredita. |
| `ARAUTO_DOWNLOAD_DIR` | A pasta com os conectores servidos em `/v1/download/`. A imagem já traz. |

## Os comandos, em qualquer instalação

```
arauto-hub init                 cria a chave-mestra, a marca, a organização e a chave do hub
arauto-hub serve                roda o hub
arauto-hub health               pergunta ao hub desta máquina se ele está no ar (0 = está)
arauto-hub migrate --i-have-a-backup   aplica migrações num banco que já tem dados
arauto-hub admin ...            o que o painel faria, por comando
```

A sequência de um primeiro uso está no guia de cada pasta. O [roteiro do primeiro teste](primeiro-teste.md) traz, passo a passo, o que conferir numa instalação nova.
