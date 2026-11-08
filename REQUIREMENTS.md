# YUGI Build Requirements

This document specifies the system, build, and runtime dependencies for building and running YUGI (Codex).

---

## System Requirements

### Operating Systems

| OS | Version | Support |
|---|---|---|
| **macOS** | 12+ (Monterey or later) | Full support (x86_64, ARM64) |
| **Linux** | Ubuntu 20.04+, Debian 10+, or other glibc-based systems | Full support (x86_64, ARM64) |
| **Windows** | 11 via WSL2 | Full support (via Windows Subsystem for Linux 2) |

### RAM and Disk Space

- **Minimum RAM**: 4 GB
- **Recommended RAM**: 8 GB or more
- **Disk Space**: 10+ GB (for build artifacts, node_modules, and Rust target directory)

### CPU

- **Minimum**: 2 cores
- **Recommended**: 4+ cores for faster builds

---

## Build Prerequisites

### Language Runtimes

#### Rust Toolchain

- **Minimum Version**: 1.97.0
- **Installation**: [https://rustup.rs/](https://rustup.rs/)
- **Components Required**:
  - `rustfmt` (code formatting)
  - `clippy` (linting)
- **Command**:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
  rustup component add rustfmt clippy
  ```

#### Node.js

- **Minimum Version**: 22.0.0
- **Installation**: [https://nodejs.org/](https://nodejs.org/)
- **Verify**: `node --version`

#### pnpm (Node Package Manager)

- **Minimum Version**: 10.34.5
- **Installation**: `npm install -g pnpm`
- **Verify**: `pnpm --version`

### Version Control

- **Git**: 2.23+ (optional but strongly recommended)
- **Installation**: [https://git-scm.com/](https://git-scm.com/)

### Build Tools

#### On macOS

- **Xcode Command Line Tools**: Required for C compilation
- **Installation**:
  ```bash
  xcode-select --install
  ```
- **Components**: clang, make, libtool

#### On Linux (Ubuntu/Debian)

- **Build Essentials**: Compilers and build infrastructure
  ```bash
  sudo apt-get update
  sudo apt-get install -y build-essential pkg-config libcap-dev libasound2-dev bubblewrap
  ```
- **Required Packages**:
  - `build-essential` (gcc, g++, make)
  - `pkg-config` (library detection)
  - `libcap-dev` (Linux capability headers for bubblewrap)
  - `libasound2-dev` (ALSA audio support)
  - `bubblewrap` (sandboxing, optional for some features)

#### On Linux (Other Distributions)

For non-Debian-based distributions (Fedora, Arch, etc.):

- `gcc`, `g++`, `make`, `pkg-config`
- `libcap-devel` or `libcap-dev` (depending on package manager)
- Development headers for audio libraries (if needed)

---

## Workspace Structure

### Rust Workspace

- **Location**: `codex-rs/`
- **Type**: Monorepo with ~100+ crates
- **Key Crates**:
  - `codex-cli`: Main CLI binary
  - `codex-tui`: Terminal user interface
  - `app-server`: Application server
  - `exec-server`: Execution server
  - `codex-bwrap`: Bubblewrap integration (Linux)
  - And many support crates for various features

### Node.js Workspace (pnpm)

- **Location**: Root directory with `pnpm-workspace.yaml`
- **Packages**:
  - `codex-cli`: npm distribution package
  - `codex-rs/responses-api-proxy/npm`: API proxy
  - `sdk/typescript`: TypeScript SDK

---

## Build-Time Dependencies

### Environment Variables

Standard build-time environment variables are documented in `FLAGS.md`.

### pkg-config Integration

YUGI uses `pkg-config` to locate system libraries, particularly on Linux:

- **Libcap**: Required for bubblewrap integration on Linux
- **Audio Libraries**: For audio feature support

#### Configuring pkg-config

On Linux, the build system automatically includes:
- `/usr/lib/x86_64-linux-gnu/pkgconfig`
- `/usr/lib/pkgconfig`
- `/usr/local/lib/pkgconfig`

Custom paths can be specified via `PKG_CONFIG_PATH`:

```bash
export PKG_CONFIG_PATH="/path/to/custom/pkgconfig:$PKG_CONFIG_PATH"
./build.sh
```

### Cross-Compilation

For cross-compilation to different Linux targets (e.g., ARM64 on x86_64):

- Install the corresponding Rust target: `rustup target add aarch64-unknown-linux-gnu`
- Ensure corresponding system libraries are available via pkg-config
- Set `PKG_CONFIG_SYSROOT_DIR` if needed: `export PKG_CONFIG_SYSROOT_DIR=/path/to/sysroot`

---

## Runtime Requirements

### macOS Runtime

- **macOS**: 12+ (matches build target)
- **Architecture**: x86_64 or ARM64 (Apple Silicon)
- **System Libraries**: Provided by macOS itself

### Linux Runtime

- **Operating System**: Ubuntu 20.04+, Debian 10+, or other glibc-based systems
- **glibc**: 2.31+
- **System Libraries**:
  - `libcap.so.2` (Linux capability support)
  - `libssl.so` (optional, for TLS features)
  - Audio libraries (if audio features are used)
- **bubblewrap**: Recommended for sandboxing (available as system package or included)

### Windows Runtime

- **Windows**: 11+
- **WSL2**: Ubuntu or Debian environment within WSL2

### Minimum Runtime Dependencies

Most runtime dependencies are statically linked or bundled:
- No additional Node.js runtime required for the bundled CLI
- All Rust dependencies are compiled into the binary

---

## Optional Dependencies

### Development Tools

These are recommended for development but not required for a basic build:

- **just**: Command runner (`cargo install --locked just`)
- **cargo-nextest**: Faster test runner (`cargo install --locked cargo-nextest`)
- **cargo-clippy**: Linter (included in Rust via `rustup component add clippy`)
- **dotslash**: Tool versioning (`cargo install --locked dotslash`)

### Code Quality Tools

- **prettier**: JavaScript formatting (`npm install -D prettier`)
- **rustfmt**: Rust formatting (included in Rust)
- **clippy**: Rust linting (included in Rust)

### Profiling and Debugging

- **perf**: Linux performance profiling
- **lldb**: Debugger for macOS/Linux
- **gdb**: Debugger for Linux

---

## Verification Checklist

Before building, verify:

- [ ] Rust toolchain installed (`rustc --version`, `cargo --version`)
- [ ] Node.js ≥22 (`node --version`)
- [ ] pnpm ≥10.34.5 (`pnpm --version`)
- [ ] On Linux: `build-essential` and `pkg-config` installed
- [ ] On Linux: `libcap-dev` installed
- [ ] Sufficient disk space (10+ GB)
- [ ] Sufficient RAM (4+ GB, 8+ GB recommended)

Run this command to check all prerequisites:

```bash
./build.sh
```

If any prerequisites are missing, the build script will provide installation instructions.

---

## Quick Start

Once all prerequisites are met:

```bash
# Clone the repository
git clone https://github.com/openai/codex.git
cd codex

# Run the canonical build
./build.sh

# Or with options
./build.sh --verbose          # See all compilation output
./build.sh --check-only       # Only check (no full build)
./build.sh --no-pnpm          # Skip Node.js build
./build.sh --no-cargo         # Skip Rust build
```

---

## Troubleshooting

### "libcap not available via pkg-config"

**Cause**: Missing libcap development headers (Linux)

**Solution**:
```bash
sudo apt-get install libcap-dev
```

### "pkg-config not found"

**Cause**: Missing pkg-config utility (Linux)

**Solution**:
```bash
sudo apt-get install pkg-config
```

### "Cannot find C compiler"

**Cause**: Missing build-essential on Linux

**Solution**:
```bash
sudo apt-get install build-essential
```

### Build runs out of memory

**Cause**: Insufficient RAM or too many parallel jobs

**Solution**:
```bash
# Limit parallel jobs
export CARGO_BUILD_JOBS=2
./build.sh
```

### pnpm install fails

**Cause**: Outdated pnpm version or corrupted cache

**Solution**:
```bash
pnpm install -g pnpm@latest
rm -rf pnpm-lock.yaml node_modules
./build.sh
```

---

## Additional Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [Node.js Documentation](https://nodejs.org/docs/)
- [pnpm Documentation](https://pnpm.io/)
- [YUGI Contributing Guide](./docs/contributing.md)
- [YUGI Installation Guide](./docs/install.md)
