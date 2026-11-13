# Arauto on Kubernetes

**The manifests have not been written yet.** The project does not publish a file that nobody ran.
This page lists what they need to guarantee, for whoever wants to write them from the image of
[`../docker/`](../docker/README.md).

## What the manifests need to guarantee

| Point | How it is on Kubernetes |
|---|---|
| **One copy of the hub.** | `replicas: 1`, with strategy `Recreate`. The send ceiling and the counters live in the memory of the process; two copies of the hub in the same organization are not handled in the first MVP ([docs/15-mvp.md](../../docs/15-mvp.md), section 2). |
| **The data folder on a volume of its own.** | A `PersistentVolumeClaim` only for `/var/lib/arauto`, outside the backup of the database. It has the master key. |
| **The database with forced write.** | PostgreSQL 16 with `fsync` on and a disk that honors the write. The hub refuses to start without this. |
| **The `init` once.** | A `Job` that runs `arauto-hub init` before the first `Deployment`. |
| **Only the port of the connectors goes out.** | A `Service` and an `Ingress` for port 8443, with websocket and idle timeout above 90 seconds. Port 8444 stays on an internal `Service`, with a `NetworkPolicy` that only lets the WhatsApp engine in. The port of the panel (8445), if you turn it on, stays on an internal `Service`, with no `Ingress`, reached by `kubectl port-forward`. |
| **The address of the connectors.** | `ARAUTO_CLIENT_IP_SOURCE=forwarded` and `ARAUTO_TRUSTED_PROXIES` with the network of the ingress controller. |
| **The health.** | `exec` with `arauto-hub health`, as readiness and as liveness. |
| **The stop.** | `terminationGracePeriodSeconds` of 20 or more. |
| **The password of the database.** | In a `Secret`, read to build `ARAUTO_DATABASE_URL`. |
| **The user.** | The image runs as user 10001. `fsGroup: 10001` on the volume of the data folder. |

## What to check

The same checks as at the end of the [Docker guide](../docker/README.md): health, internal port
closed to the outside, download with the SHA-256 of the block, address of the connectors and
orderly stop.
