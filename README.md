# Arcadia Steam Extension

A Rust library extension for integrating Steam game library functionality into the Arcadia platform.

## Features

- Scan Steam game libraries
- Retrieve detailed game information
- Launch Steam games
- Cross-platform support (Windows, macOS, Linux)

## Development

### Prerequisites

- Rust 1.70+ (2024 edition)
- Cargo

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
```

### Linting

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## CI/CD

This project uses GitHub Actions for continuous integration:

- **Clippy**: Runs on pull requests to main with warnings treated as errors
- **Tests**: Executes all unit tests
- **Build**: Verifies cross-platform compilation on Ubuntu, macOS, and Windows

## Branch Protection

To ensure code quality, the following branch protection rules are recommended:

1. **Require pull request reviews before merging**
2. **Require status checks to pass before merging**
   - CI workflow must pass
3. **Require branches to be up to date before merging**
4. **Require linear history**
5. **Include administrators** in the above requirements

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

MIT
