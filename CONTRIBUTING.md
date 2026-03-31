# Contributing to Telegram Drive

Thanks for considering a contribution! This guide covers the basics.

## Prerequisites

- **Rust** 1.78+ (stable)
- **Node.js** 20+ with npm
- A Telegram account and API credentials for integration testing

## Repository layout

```
server/     Rust (Actix-Web) backend
web/        React + Vite frontend
deploy/     Docker, systemd, nginx configs
docs/       Architecture & API docs
```

## Getting started

```bash
# Backend
cp .env.example server/.env       # edit with your Telegram creds
cd server
cargo build
cargo test

# Frontend
cd web
npm install
npm run dev
```

## Code style

- **Rust**: run `cargo fmt` and `cargo clippy` before committing.
- **TypeScript**: run `npm run lint` (ESLint + Prettier).

## Branching

1. Fork the repository and create a feature branch from `main`.
2. Keep commits small and focused.
3. Open a pull request describing what changed and why.

## Testing

- Backend: `cargo test` — all tests must pass.
- Frontend: `npm run build` — must compile without errors.
- Add tests for new endpoints or services where practical.

## Security

If you discover a security vulnerability, please report it privately
instead of opening a public issue. See the README for contact info.

## License

By contributing you agree that your contributions will be licensed under the
project's existing license.
