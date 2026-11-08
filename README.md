# YUGI — Claude Code Agent

**YUGI** is a Rust-based implementation of Claude Code, an AI-powered coding agent that runs locally and operates directly in your development environment.

## Quick Start

Clone and build from source:

```bash
git clone https://github.com/openai/codex.git
cd codex/codex-rs

# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Install dependencies
cargo install --locked just
cargo install --locked cargo-nextest

# Build and run
cargo build --release
./target/release/yugi "explain this codebase to me"
```

For detailed setup instructions, see [GETTING_STARTED.md](./GETTING_STARTED.md).

## What is YUGI?

YUGI (the binary name for `codex-cli`) is an AI agent that:
- Understands your codebase structure and context
- Executes tasks: refactoring, testing, debugging, code review
- Integrates with local tools: git, cargo, npm, shell commands
- Runs entirely on your machine with configurable AI models
- Provides a TUI and programmatic interfaces

## Features

- **Local-first**: Full control over code and model selection
- **Modular**: 100+ crates with specialized responsibilities
- **Extensible**: Plugin system for custom skills and integrations
- **Hardened**: Process isolation, permission system, audit logging

## System Requirements

| Requirement | Details |
|-------------|---------|
| OS | macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 (WSL2) |
| RAM | 4 GB minimum (8 GB recommended) |
| Git | 2.23+ recommended |

## Building

### Prerequisites

- Rust 1.99+ (installed via rustup)
- Python 3.9+
- Standard development tools (gcc/clang, make)

### From Source

```bash
cd codex/codex-rs
cargo build --release

# Validate build
cargo test -p codex-cli

# Run quick tests
just test
```

See [GETTING_STARTED.md](./GETTING_STARTED.md) for step-by-step instructions.

## Known Limitations

- **V8 Pre-built Artifacts**: CI uses pre-built rusty_v8 bindings; local builds may require additional setup
- **Platform Coverage**: Primary testing on Linux x86_64; macOS and Windows support varies
- **Dependencies**: GStreamer and other system libs must be available; versions may conflict
- **Bazel Removal**: This fork removed Bazel infrastructure; uses pure Cargo/Rust builds

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) for workarounds and [KNOWN_ISSUES.md](./KNOWN_ISSUES.md) for detailed tracking.

## Project Structure

```
codex-rs/
├── cli/              # Main YUGI binary (codex-cli crate)
├── app-server/       # Protocol and service layer
├── core/             # Core agent logic
├── exec/             # Command execution sandbox
├── ext/              # Extensions (MCP, skills, web-search, etc.)
├── hooks/            # Plugin system for custom behavior
└── [90+ supporting crates]
```

## Development

### Formatting & Linting

```bash
just fmt              # Format code
just fix -p <crate>  # Run clippy on a crate
```

### Testing

```bash
just test            # Run full test suite via nextest
just test -p cli     # Run tests for one crate
```

### Logging & Debugging

```bash
RUST_LOG=debug ./target/debug/yugi <args>
./target/debug/yugi -c log_dir=./logs <args>
```

## Documentation

- [GETTING_STARTED.md](./GETTING_STARTED.md) — 15-minute quickstart for new developers
- [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) — Common issues and fixes
- [KNOWN_ISSUES.md](./KNOWN_ISSUES.md) — Detailed issue tracking and workarounds
- [docs/install.md](./docs/install.md) — Detailed build instructions
- [docs/contributing.md](./docs/contributing.md) — Contribution guidelines

## Contributing

We do not accept external pull requests. Community contributions are welcome as:
- Detailed bug reports with reproduction steps
- Root-cause analysis and investigation
- Feature requests and design discussions

See [docs/contributing.md](./docs/contributing.md) for details.

## License

Apache License 2.0 — See [LICENSE](./LICENSE)

## Support

- **Issues**: Report bugs on [GitHub Issues](https://github.com/openai/codex/issues)
- **Docs**: Comprehensive guides in `/docs` and `/GETTING_STARTED.md`
- **CI Status**: [GitHub Actions](https://github.com/openai/codex/actions)
