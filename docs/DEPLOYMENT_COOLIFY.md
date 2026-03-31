# Deploy with Coolify

> Security rule: never commit real secrets, private IPs, tunnel tokens, or internal hostnames to git. Use placeholders in docs and set real values only in Coolify/Cloudflare UIs.

This guide deploys Telegram Drive as a **Docker Compose stack** in Coolify using:

- `server` (Rust API)
- `web` (Vite static build served by nginx, reverse-proxying `/api` to `server`)

## 1. Repository setup in Coolify

1. Create a new **Application** in Coolify.
2. Choose **Docker Compose** deployment.
3. Select compose file path:
   - `deploy/docker/docker-compose.coolify.yml`

## 2. Required environment variables

Set these in Coolify (use the single contract in `.env.example` as reference):

- `CORS_ALLOWED_ORIGIN` (example: `https://drive.example.com`)
- `SESSION_SECRET`
- `ADMIN_PASSWORD`
- `TELEGRAM_API_ID`
- `TELEGRAM_API_HASH`

Optional:

- `SESSION_TTL_HOURS` (default `8`)
- `RUST_LOG` (default `info`)
- `LOG_FORMAT` (default `json`)

Set these values only in Coolify environment settings (or another secret manager). Do not hardcode production secrets into compose files committed to git.

## 3. Domain setup

1. In Coolify, assign your domain to service **`web`**.
2. Keep service port as `80` (internal container port).
3. Ensure TLS is enabled in Coolify.

## 4. Persistent storage

The stack uses volume `tdw-data` mounted at `/app/data` in the backend.
It stores:

- admin hash (`admin.json`)
- Telegram session
- cached media files

The backend image startup script creates/chowns these directories on boot and then runs the server as non-root (`tdrive`), so first deploy works even with fresh volumes.

## 5. Health checks

Health checks are defined in compose:

- `server`: `GET /api/health`
- `web`: nginx root check

## 6. Smoke test after deploy

1. Open your assigned domain.
2. Login with `ADMIN_PASSWORD`.
3. Run Telegram connect flow.
4. Verify API health via browser network tab calling `/api/health` through the same domain.

## 7. Local dry-run (optional)

You can validate the compose file locally:

```bash
cd /home/telegram/Telegram-Drive
cp .env.example .env.coolify
# edit secrets in .env.coolify
docker compose --env-file .env.coolify -f deploy/docker/docker-compose.coolify.yml config
```

If you want to run it locally without Coolify ingress, temporarily publish web port in compose:

```yaml
web:
  ports:
    - "8080:80"
```

Then access `http://localhost:8080`.

## 8. Split topology with external Cloudflared tunnel

If your edge tunnel runs outside the Docker host, keep Coolify as the application platform and point Cloudflared to the Coolify proxy entrypoint.

- Use the same public hostname in both places:
  - Cloudflared ingress `hostname`
  - Coolify domain attached to service `web`
- Ensure Cloudflared forwards the correct `Host` header to Coolify (host-based routing depends on this).
- Keep `CORS_ALLOWED_ORIGIN` aligned with the final public URL.

See the complete guide and templates in [DEPLOYMENT_SPLIT_CLOUDFLARED.md](DEPLOYMENT_SPLIT_CLOUDFLARED.md).
