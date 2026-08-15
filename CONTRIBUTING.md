# Contributing

Thank you for contributing to DramaDNA.

## Development setup

Follow the prerequisites and setup steps in [README.md](README.md). Keep changes focused, avoid committing generated files or real media, and use synthetic data in tests and examples.

## Before submitting a pull request

```bash
pnpm install --frozen-lockfile
pnpm build
pnpm audit --prod
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Pull requests should explain the user-visible change, testing performed and any migration or compatibility impact. Never include real API keys, user databases, private video material, signing credentials or machine-specific paths.

By submitting a contribution, you agree that it is licensed under the repository's [MIT License](LICENSE).
