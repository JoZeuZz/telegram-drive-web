# Task Completion Checklist

## After making changes:
1. Run `npm run build` in `app/` to check TypeScript compilation
2. Run `cargo check` in `app/src-tauri/` for Rust changes
3. Test with `npm run tauri dev` for full integration

## No linter/formatter configured
- No ESLint or Prettier in dependencies
- No cargo fmt/clippy in CI
- TypeScript compiler is the main check (strict mode)
