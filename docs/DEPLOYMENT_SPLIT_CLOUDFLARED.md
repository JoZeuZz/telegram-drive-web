# Split Deployment: Coolify + External Cloudflared Tunnel

This guide documents a split topology where:

- Coolify manages app deployment (Docker Compose stack)
- Cloudflared runs in a separate node and exposes the public domain
- Traffic is forwarded to the Coolify proxy (Traefik)

Use this guide when your edge tunnel host is different from your Docker/Coolify host.

## Security policy (mandatory)

- Never commit real secrets, private IPs, internal hostnames, tunnel tokens, or credentials.
- Keep runtime values only in secret managers and deployment UIs.
- Use placeholders in files and screenshots (`example.com`, `<your-secret>`, `***REDACTED***`).

## 1. Target topology

```text
Browser -> Cloudflare Edge -> Cloudflared -> Coolify/Traefik -> web service -> server API
```

Key behavior:

- Cloudflared forwards the request to Coolify proxy.
- Coolify routes to the correct app using the `Host` header.
- Backend CORS allows only your public URL.

## 2. Prepare application in Coolify

1. Create a Docker Compose application using:
   - `deploy/docker/docker-compose.coolify.yml`
2. Configure required environment variables in Coolify UI:
   - `APP_ENV=production`
   - `CORS_ALLOWED_ORIGIN`
   - `COOKIE_SECURE=true`
   - `SESSION_SECRET`
   - `ADMIN_PASSWORD`
   - `TRUST_PROXY_HEADERS=true` (recommended in this topology)
   - `TELEGRAM_API_ID`
   - `TELEGRAM_API_HASH`
   - Use root `.env.example` as the single contract reference for names/defaults.
3. Attach the public domain to service `web` in Coolify.

Important: the domain configured in Coolify must match the hostname configured in Cloudflared.

## 3. Prepare Cloudflared node

1. Install cloudflared.
2. Authenticate and create tunnel (Cloudflare account required).
3. Create tunnel DNS route for your public hostname.
4. Use the config template from `deploy/cloudflared/config.yml.example`.
5. Run cloudflared as a system service.

Example command flow (generic):

```bash
cloudflared tunnel login
cloudflared tunnel create teldrive
cloudflared tunnel route dns teldrive teldrive.example.com
```

## 4. Cloudflared config notes

Template highlights:

- `hostname`: public domain users access.
- `service`: private URL of Coolify proxy reachable from cloudflared node.
- `originRequest.httpHostHeader`: must be the same public hostname so Coolify routes correctly.

If you route to an HTTPS origin with private certificates, configure trust explicitly. Avoid disabling TLS verification unless strictly necessary.

## 5. Required value alignment

Ensure these three values are aligned exactly:

1. Public DNS hostname (Cloudflare DNS / tunnel route)
2. Coolify domain bound to service `web`
3. `CORS_ALLOWED_ORIGIN` in app environment

If these differ, you may get routing or CORS failures.

## 6. Smoke test checklist

1. Confirm tunnel status healthy in Cloudflare Zero Trust.
2. Open the public URL and load the web UI.
3. Log in with `ADMIN_PASSWORD`.
4. Check browser network call to `/api/health` returns `200`.
5. Complete Telegram connect flow.

## 7. Common issues

| Symptom | Likely cause | Fix |
|---|---|---|
| `404` from Coolify proxy | Host header mismatch | Set `originRequest.httpHostHeader` to the public hostname |
| Browser CORS errors | Wrong `CORS_ALLOWED_ORIGIN` | Set exact `https://your-domain` |
| Session cookie not persisted | Missing HTTPS at edge | Ensure public endpoint is HTTPS and `COOKIE_SECURE=true` |
| Tunnel healthy but app unreachable | Cloudflared cannot reach Coolify proxy URL | Verify internal routing/firewall and service URL |

## 8. Related docs

- [DEPLOYMENT_COOLIFY.md](DEPLOYMENT_COOLIFY.md)
- [DEPLOYMENT_LXC.md](DEPLOYMENT_LXC.md)
- [SECURITY.md](SECURITY.md)
