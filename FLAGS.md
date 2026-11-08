# YUGI Build Environment Flags

This document specifies all environment variables and flags that control the behavior of YUGI's build system.

---

## Build Script Flags

### Command-Line Options for `build.sh`

These are command-line arguments passed directly to `./build.sh`:

```bash
./build.sh [OPTIONS]
```

| Option | Purpose | Default |
|--------|---------|---------|
| `--help` | Display help message and usage | (shows help) |
| `--check-only` | Run `cargo check --lib` without full build | `false` |
| `--no-cargo` | Skip Cargo/Rust build steps | `false` |
| `--no-pnpm` | Skip pnpm install and build steps | `false` |
| `--verbose` | Print all compiler output (verbose mode) | `false` |

### Examples

```bash
# Run full build
./build.sh

# Only check Rust library compilation
./build.sh --check-only

# Skip pnpm, build Rust only
./build.sh --no-pnpm

# Show all compilation output
./build.sh --verbose

# Check only, verbose
./build.sh --check-only --verbose
```

---

## Cargo/Rust Build Environment Variables

These environment variables control Rust/Cargo build behavior:

### Core Build Variables

#### `CARGO_BUILD_JOBS`

- **Purpose**: Number of parallel compiler jobs
- **Default**: Auto-detected (from `nproc` or system CPU count)
- **Type**: Integer
- **Example**: `export CARGO_BUILD_JOBS=4`
- **Note**: Set this to reduce memory usage on systems with limited RAM

#### `CARGO_INCREMENTAL`

- **Purpose**: Enable incremental compilation (faster rebuilds after changes)
- **Default**: Unset (uses Cargo's default)
- **Type**: 0 (disabled) or 1 (enabled)
- **Example**: `export CARGO_INCREMENTAL=0` (disabled for CI/clean builds)
- **Note**: GitHub Actions workflows disable this for cleaner builds

#### `RUST_BACKTRACE`

- **Purpose**: Control panic backtrace output
- **Default**: Unset (minimal backtraces)
- **Type**: `0` (off), `1` (minimal), `full` (complete)
- **Example**: `export RUST_BACKTRACE=1` or `RUST_BACKTRACE=full`
- **Use Case**: Debugging build or runtime failures

#### `RUST_LOG`

- **Purpose**: Filter logging output from the application
- **Default**: Unset (no logging)
- **Type**: Logging filter string (e.g., `debug`, `info`, `warn`)
- **Example**: `export RUST_LOG=codex=debug`
- **Documentation**: [env_logger docs](https://docs.rs/env_logger/latest/env_logger/#enabling-logging)

---

### Linking and Compilation

#### `RUSTFLAGS`

- **Purpose**: Pass additional flags to the Rust compiler
- **Default**: Empty
- **Type**: String of compiler flags
- **Example**: `export RUSTFLAGS="-C opt-level=3"`
- **Common Uses**:
  - `-C opt-level=0`: Minimal optimization (fast compile)
  - `-C opt-level=3`: Maximum optimization (slower compile, faster runtime)
  - `-C target-cpu=native`: Optimize for current CPU
  - `-C link-arg=-fuse-ld=lld`: Use LLVM linker

#### `RUST_MIN_STACK`

- **Purpose**: Minimum stack size for spawned threads
- **Default**: Not set (8 MiB minimum in justfile)
- **Type**: Integer (bytes)
- **Example**: `export RUST_MIN_STACK=8388608` (8 MB)
- **Note**: Set in `justfile` for CI; set manually for debugging

#### `CC` and `CXX`

- **Purpose**: Specify C and C++ compilers for build.rs scripts
- **Default**: Auto-detected (gcc/g++ on Linux, clang on macOS)
- **Type**: Path to compiler
- **Example**: `export CC=clang CXX=clang++`
- **Use Case**: Cross-compilation or specific compiler requirements

---

### pkg-config (System Libraries)

#### `PKG_CONFIG_PATH`

- **Purpose**: Search paths for .pc files (pkg-config library metadata)
- **Default**: Standard system paths (auto-configured by `build.sh`)
- **Type**: Colon-separated list of paths
- **Example**: `export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/custom/lib/pkgconfig"`
- **Used By**: `codex-bwrap` (libcap), audio features
- **Note**: `build.sh` automatically includes standard paths on Linux

#### `PKG_CONFIG_SYSROOT_DIR`

- **Purpose**: Root directory for cross-compilation pkg-config searches
- **Default**: Not set (native compilation)
- **Type**: Filesystem path
- **Example**: `export PKG_CONFIG_SYSROOT_DIR="/opt/sysroot/aarch64"`
- **Use Case**: Cross-compilation to different CPU architectures
- **Related**: `PKG_CONFIG_PATH_<TARGET>` for target-specific paths

#### `PKG_CONFIG_ALLOW_CROSS`

- **Purpose**: Allow pkg-config to find libraries for cross-compilation targets
- **Default**: Not set
- **Type**: Boolean (set to any value to enable)
- **Example**: `export PKG_CONFIG_ALLOW_CROSS=1`
- **Note**: Required when cross-compiling (e.g., x86_64 → ARM64)

#### `PKG_CONFIG_LIBDIR`

- **Purpose**: Override all library search directories
- **Default**: Not set
- **Type**: Colon-separated list
- **Example**: `export PKG_CONFIG_LIBDIR="/opt/custom/lib/pkgconfig"`
- **Warning**: Setting this clears standard paths; prefer `PKG_CONFIG_PATH` instead

---

### Memory and Caching

#### `MALLOC_CONF`

- **Purpose**: Configure jemalloc memory allocator behavior
- **Default**: `thp:always` (from `.cargo/config.toml`)
- **Type**: jemalloc configuration string
- **Example**: `export MALLOC_CONF="thp:always"`
- **Documentation**: [jemalloc tuning](https://jemalloc.net/jemalloc.3.html)
- **Note**: Enables transparent huge pages for better performance

#### `SCCACHE_CACHE_SIZE`

- **Purpose**: Maximum cache size when using sccache (distributed cache)
- **Default**: Not set
- **Type**: Size string (e.g., `10G`, `5000M`)
- **Example**: `export SCCACHE_CACHE_SIZE=10G`
- **Use Case**: CI/CD with shared build cache

#### `SCCACHE_DIR`

- **Purpose**: Directory for sccache cache storage
- **Default**: OS-dependent default cache directory
- **Type**: Filesystem path
- **Example**: `export SCCACHE_DIR=/tmp/sccache`

---

### Target and Profile

#### `CARGO_CFG_TARGET_OS`

- **Purpose**: Override target OS (auto-detected by Cargo)
- **Default**: Current system OS
- **Type**: `linux`, `macos`, `windows`, etc.
- **Note**: Usually auto-set by Cargo; rarely overridden manually

#### `CODEX_CI_TARGET`

- **Purpose**: YUGI-specific: specify cross-compilation target
- **Default**: Host target
- **Type**: Rust target triple (e.g., `aarch64-unknown-linux-gnu`)
- **Example**: `export CODEX_CI_TARGET=aarch64-unknown-linux-gnu`
- **Used By**: GitHub Actions workflows

#### `CODEX_CI_PROFILE`

- **Purpose**: YUGI-specific: build profile (debug/release)
- **Default**: `debug`
- **Type**: `debug` or `release`
- **Example**: `export CODEX_CI_PROFILE=release`
- **Used By**: GitHub Actions workflows

---

### Bwrap (Linux Bubblewrap) Build

#### `CODEX_SKIP_BWRAP_BUILD`

- **Purpose**: Skip bubblewrap compilation (Linux only)
- **Default**: Not set (build on Linux if source available)
- **Type**: Boolean (set to any value to enable)
- **Example**: `export CODEX_SKIP_BWRAP_BUILD=1`
- **Use Case**: Build without sandboxing support
- **Note**: Only affects Linux builds; other platforms ignore

#### `CODEX_BWRAP_SOURCE_DIR`

- **Purpose**: Path to custom bubblewrap source tree
- **Default**: Uses vendored bubblewrap in `codex-rs/vendor/bubblewrap`
- **Type**: Filesystem path
- **Example**: `export CODEX_BWRAP_SOURCE_DIR=/opt/bubblewrap-src`
- **Use Case**: Build with custom bubblewrap version
- **Note**: Only on Linux if bubblewrap compilation is enabled

---

## pnpm/Node.js Environment Variables

### Core pnpm Variables

#### `PNPM_HOME`

- **Purpose**: Directory for pnpm global installations
- **Default**: `~/.pnpm-global` or OS-dependent
- **Type**: Filesystem path
- **Example**: `export PNPM_HOME="$HOME/.pnpm"`

#### `PNPM_STORE_DIR`

- **Purpose**: Directory for pnpm package store
- **Default**: `.pnpm-store` in workspace root or global
- **Type**: Filesystem path
- **Example**: `export PNPM_STORE_DIR="/var/cache/pnpm"`

#### `npm_config_registry`

- **Purpose**: NPM package registry URL
- **Default**: `https://registry.npmjs.org/`
- **Type**: URL
- **Example**: `export npm_config_registry=https://custom.registry.org/`

#### `NODE_OPTIONS`

- **Purpose**: Pass options to Node.js runtime
- **Default**: Not set
- **Type**: Node.js flags (e.g., `--max-old-space-size=4096`)
- **Example**: `export NODE_OPTIONS="--max-old-space-size=4096"`
- **Use Case**: Increase memory for large builds

---

## CI/CD and Special Variables

### GitHub Actions

#### `GITHUB_ENV`

- **Purpose**: File for persisting environment variables across workflow steps
- **Default**: Set by GitHub Actions runner
- **Type**: Filesystem path
- **Note**: Used by `.github/scripts/install-musl-build-tools.sh`

#### `GITHUB_WORKSPACE`

- **Purpose**: Workspace directory in GitHub Actions
- **Default**: Set by GitHub Actions runner
- **Type**: Filesystem path

### Repository Root

#### `CODEX_REPO_ROOT`

- **Purpose**: Root directory of the YUGI repository
- **Default**: Auto-detected by `build.sh`
- **Type**: Filesystem path
- **Example**: Set by `build.sh` to `$(cd "$(dirname build.sh)" && pwd)`
- **Used By**: `justfile`, build scripts

---

## Cross-Compilation Variables

### For musl libc (static builds)

When building with `x86_64-unknown-linux-musl` or `aarch64-unknown-linux-musl`:

#### `TARGET`

- **Purpose**: Rust target triple
- **Example**: `export TARGET=x86_64-unknown-linux-musl`

#### `OPENSSL_CC`

- **Purpose**: C compiler for musl OpenSSL build
- **Default**: Auto-detected by `.github/scripts/install-musl-openssl.sh`
- **Type**: Path to C compiler
- **Example**: `export OPENSSL_CC=x86_64-linux-musl-gcc`

#### `PKG_CONFIG_SYSROOT_DIR`

- **Purpose**: Root of musl sysroot
- **Default**: Not set
- **Type**: Filesystem path
- **Example**: `export PKG_CONFIG_SYSROOT_DIR=/opt/musl`

---

## Diagnostic and Debug Variables

#### `RUST_BACKTRACE`

- **Purpose**: Enable Rust panic backtraces
- **Value**: `0` (off), `1` (minimal), `full` (complete)
- **Example**: `export RUST_BACKTRACE=full`

#### `RUST_PANIC`

- **Purpose**: Control panic behavior
- **Default**: `unwind` (stack unwinding)
- **Type**: `unwind` or `abort`
- **Example**: `export RUST_PANIC=abort`

#### `VERBOSE`

- **Purpose**: Pass to `build.sh` for detailed output
- **Method**: `./build.sh --verbose`

#### `RUST_DEBUG`

- **Purpose**: Enable Rust compiler debug output
- **Default**: Not set
- **Type**: Boolean (set to any value)
- **Example**: `export RUST_DEBUG=1`

---

## Quick Reference Tables

### Common Build Commands

```bash
# Standard build
./build.sh

# Fast check (library only)
./build.sh --check-only

# Verbose build with limited parallelism
CARGO_BUILD_JOBS=2 ./build.sh --verbose

# Release build (add to cargo)
cargo build --release

# With debugging symbols and backtraces
RUST_BACKTRACE=full cargo build

# Cross-compile to ARM64 (requires target installed)
rustup target add aarch64-unknown-linux-gnu
CODEX_CI_TARGET=aarch64-unknown-linux-gnu ./build.sh
```

### Typical Development Workflow

```bash
# Initial setup
export CARGO_BUILD_JOBS=4
export RUST_LOG=debug
export RUST_BACKTRACE=1

# Run build
./build.sh

# Run tests with logging
RUST_LOG=codex=debug cargo test

# Quick recompile after changes
cargo build
```

### CI/CD Settings

```bash
# For GitHub Actions
export CARGO_INCREMENTAL=0
export CODEX_CI_PROFILE=release
export PKG_CONFIG_ALLOW_CROSS=1

# Build with release profile
cargo build --release
```

---

## Summary of Key Paths

| Variable | Purpose | Example |
|----------|---------|---------|
| `CODEX_REPO_ROOT` | Repository root | `/home/user/YUGI` |
| `PKG_CONFIG_PATH` | System library paths | `/usr/lib/pkgconfig` |
| `CARGO_HOME` | Cargo cache directory | `~/.cargo` |
| `RUST_TOOLCHAIN` | Active Rust version | (via `rustup`) |

---

## Notes

- All variables are **optional** unless explicitly noted
- Environment variables are inherited by child processes
- Use `export VAR=value` to set variables for subcommands
- Unset variables with `unset VAR`
- View all exported variables with `env | grep -E '^(CARGO|RUST|PKG_CONFIG|CODEX)'`

---

## References

- [Cargo Environment Variables](https://doc.rust-lang.org/cargo/guide/environment-variables.html)
- [RUSTFLAGS Documentation](https://doc.rust-lang.org/cargo/guide/build-cache.html)
- [pkg-config Manual](https://linux.die.net/man/1/pkg-config)
- [pnpm Documentation](https://pnpm.io/)
- [Node.js CLI Options](https://nodejs.org/api/cli.html)
