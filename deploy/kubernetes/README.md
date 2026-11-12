# Arauto em Kubernetes

**Os manifestos ainda não foram escritos.** O projeto não publica um arquivo que ninguém rodou. Esta
página lista o que eles precisam garantir, para quem quiser escrevê-los a partir da imagem de
[`../docker/`](../docker/README.md).

## O que os manifestos precisam garantir

| Ponto | Como fica em Kubernetes |
|---|---|
| **Uma cópia do hub.** | `replicas: 1`, com estratégia `Recreate`. O teto de envio e os contadores vivem na memória do processo; duas cópias do hub na mesma organização não são tratadas no primeiro MVP ([docs/15-mvp.md](../../docs/15-mvp.md), seção 2). |
| **A pasta de dados num volume próprio.** | Um `PersistentVolumeClaim` só para `/var/lib/arauto`, fora da cópia de segurança do banco. Ele tem a chave-mestra. |
| **O banco com gravação forçada.** | PostgreSQL 16 com `fsync` ligado e disco que honre a gravação. O hub se recusa a subir sem isso. |
| **O `init` uma vez.** | Um `Job` que roda `arauto-hub init` antes do primeiro `Deployment`. |
| **Só a porta dos conectores sai.** | Um `Service` e um `Ingress` para a porta 8443, com websocket e prazo de inatividade acima de 90 segundos. A porta 8444 fica num `Service` interno, com `NetworkPolicy` que só deixa o motor de WhatsApp entrar. |
| **O endereço dos conectores.** | `ARAUTO_CLIENT_IP_SOURCE=forwarded` e `ARAUTO_TRUSTED_PROXIES` com a rede do controlador de entrada. |
| **A saúde.** | `exec` com `arauto-hub health`, como prontidão e como vida. |
| **A parada.** | `terminationGracePeriodSeconds` de 20 ou mais. |
| **A senha do banco.** | Num `Secret`, lida para montar `ARAUTO_DATABASE_URL`. |
| **O usuário.** | A imagem roda como o usuário 10001. `fsGroup: 10001` no volume da pasta de dados. |

## O que conferir

As mesmas conferências do fim do [guia do Docker](../docker/README.md): saúde, porta interna
fechada para fora, download com o SHA-256 do bloco, endereço dos conectores e parada em ordem.
