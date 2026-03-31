# Cloudflared Templates

This directory contains templates for running an external Cloudflared tunnel in front of Coolify.

Files:

- `config.yml.example`: baseline tunnel config for hostname routing to Coolify proxy.

Usage:

1. Copy the template to your runtime location:
   - `/etc/cloudflared/config.yml`
2. Replace placeholders with real values in your deployment environment.
3. Keep credentials and tunnel identifiers out of git.

Reference docs:

- `docs/DEPLOYMENT_SPLIT_CLOUDFLARED.md`
- `docs/DEPLOYMENT_COOLIFY.md`
