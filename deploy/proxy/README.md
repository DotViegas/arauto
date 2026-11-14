# The proxy in front of the hub

The hub does not speak TLS: what terminates the TLS is a proxy of yours. These are examples, **not
tested**, for whoever wants a starting point. Any proxy will do, as long as it meets the three
conditions.

## What the proxy needs to do

| Condition | Why |
|---|---|
| Pass on the switch to websocket (`Upgrade`). | The channel of the connectors is a websocket at `/v1/channel`. |
| Idle timeout **above 90 seconds**. | The channel beats every 30 seconds and gives up at 90. A shorter timeout drops good connections. |
| Not hold the response in memory (no buffer). | The hub measures what goes out. A proxy that accumulates hides a slow connector. |

And a fourth, of security: the proxy **writes** the header `X-Forwarded-For` with the address of
whoever arrived, **replacing** what came from outside. The hub only believes this header when the
connection comes from an address of `ARAUTO_TRUSTED_PROXIES`.

Four paths need to reach the hub: `/v1/channel`, `/v1/download/`, and, for the connection sentence
of the panel, `/v1/join/` (the instruction page of an invite) and `/llms.txt`. The two last ones are
a proposal, built and not tried on a real Hermes. Without them the address in the sentence answers
404 at the proxy. The rest can answer 404 at the proxy itself.

The panel does **not** go in this site. It has a port of its own in the hub (8445), off by default,
and the foreseen path is a tunnel to it ([guide](../docker/README.md)). Whoever wants to open it
through a proxy gives it another name, with TLS, pointing to `127.0.0.1:8445`; the three conditions
above belong to the channel and do not apply to it.

## Caddy

```
arauto.example.org {
    @arauto path /v1/channel /v1/download/* /v1/join/* /llms.txt
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
    # ssl_certificate and ssl_certificate_key: yours.

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

    location /v1/join/ {
        proxy_pass http://127.0.0.1:8443;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
    }

    location = /llms.txt {
        proxy_pass http://127.0.0.1:8443;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
    }

    location / {
        return 404;
    }
}
```

In both examples, `X-Forwarded-For` carries **only** the address of whoever arrived at the proxy.
Do not use the form that appends to the received header: it lets whoever is outside choose the
first address of the list.
