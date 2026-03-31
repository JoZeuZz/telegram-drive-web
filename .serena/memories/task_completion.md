# Task Completion Checklist

## After making changes
1. Run `npm run build` in `web/` to validate TypeScript + Vite build.
2. Run `cargo test --release` in `server/` to validate backend behavior.
3. If backend Rust code changed, run `cargo fmt` and keep diffs focused.

## CI expectations
- GitHub Actions CI is defined in `.github/workflows/ci.yml`.
- CI checks include:
  - Rust fmt/clippy/build/tests
  - Frontend type-check + build
  - Residual `@tauri-apps` detection in `web/`
