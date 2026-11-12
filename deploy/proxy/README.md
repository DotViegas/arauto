# O proxy na frente do hub

O hub não fala TLS: quem termina o TLS é um proxy seu. Estes são exemplos, **não testados**, para
quem quer um ponto de partida. Qualquer proxy serve, desde que cumpra as três condições.

## O que o proxy precisa fazer

| Condição | Por quê |
|---|---|
| Repassar a troca para websocket (`Upgrade`). | O canal dos conectores é um websocket em `/v1/channel`. |
| Prazo de inatividade **acima de 90 segundos**. | O canal bate a cada 30 segundos e desiste aos 90. Um prazo menor derruba conexões boas. |
| Não segurar a resposta em memória (sem buffer). | O hub mede o que sai. Um proxy que acumula esconde um conector lento. |

E uma quarta, de segurança: o proxy **escreve** o cabeçalho `X-Forwarded-For` com o endereço de quem
chegou, **substituindo** o que veio de fora. O hub só acredita nesse cabeçalho quando a conexão vem
de um endereço de `ARAUTO_TRUSTED_PROXIES`.

Só dois caminhos precisam chegar ao hub: `/v1/channel` e `/v1/download/`. O resto pode responder 404
no próprio proxy.

## Caddy

```
arauto.example.org {
    @arauto path /v1/channel /v1/download/*
    handle @arauto {
        reverse_proxy 127.0.0.1:8443 {
            flush_interval -1
            header_up X-Forwarded-For {remote_host}
            transport http {
                read_timeout 120s
                write_timeout 120s
            }
        }
    }
    handle {
        respond 404
    }
}
```

## NGINX

```
server {
    listen 443 ssl;
    server_name arauto.example.org;
    # ssl_certificate e ssl_certificate_key: os seus.

    location = /v1/channel {
        proxy_pass http://127.0.0.1:8443;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_read_timeout 120s;
        proxy_send_timeout 120s;
        proxy_buffering off;
    }

    location /v1/download/ {
        proxy_pass http://127.0.0.1:8443;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
    }

    location / {
        return 404;
    }
}
```

Nos dois exemplos, `X-Forwarded-For` leva **só** o endereço de quem chegou ao proxy. Não use a forma
que acrescenta ao cabeçalho recebido: ela deixa quem está de fora escolher o primeiro endereço da
lista.
