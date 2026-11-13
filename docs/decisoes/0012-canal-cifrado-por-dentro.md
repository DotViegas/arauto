# 0012 — Channel encrypted inside the websocket

**Status:** proposal, to be validated in the prototype · **Date:** 09/11/2026

## Context

In real installations TLS ends at a proxy that does not belong to the project, and on an internal network it may not exist. Depending on TLS alone would leave the security of the connection in the hands of each installation.

## Decision

The channel uses the Noise XK pattern inside the websocket, in a fixed suite and with no fallback mode. The static key of the connector is the device identity; the hub's is the one that came in the block. At enrollment the connector carries the invite; in the following connections, the sealed credential.

## Consequences

- The security of the channel is the same with TLS, without TLS and with a third-party proxy.
- The invite only travels encrypted, to the true hub. The hub keeps only its digest.
- The connector proves it has the identity already at enrollment.
- A single handshake for enrollment and connection.
- The verification code comes out of the handshake itself and of a number drawn by the hub: both sides compute it and nobody transmits it.
- It does not protect the panel: the admin's browser depends on the TLS of whoever installs.
- Cost: implementing and testing the message format, with test vectors.
- **Recorded alternative:** TLS 1.3 with a signed challenge. It is simpler and is enough when the proxy belongs to whoever installs. It stays as plan B if the prototype shows that the cost of Noise does not pay off.
