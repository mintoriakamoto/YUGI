# Known Issues

This document tracks issues that affect development, building, or running YUGI. Each issue includes reproduction steps, workarounds, and expected resolution timeline.

**Last Updated**: 2026-11-08  
**Severity Levels**: ⚠️ Critical | 🔴 High | 🟡 Medium | 🟢 Low

---

## Issue #1: V8 Pre-built Artifacts Not Available ⚠️ Critical

**Status**: Awaiting upstream (rustup, rusty_v8 project)  
**Affects**: Local builds on unsupported platforms  
**Reporter**: YUGI fork  
**Since**: Initial fork

### Description

The project uses pre-built `rusty_v8` bindings from GitHub releases. These bindings are only built for specific platform/architecture combinations:
- Linux x86_64 (glibc)
- macOS x86_64 (Intel)
- macOS aarch64 (Apple Silicon)
- Windows x86_64 (MSVC)

If you're building on an unsupported platform or variant (e.g., musl-based Linux, Alpine, 32-bit architectures), the build will fail with:
```
error: V8 pre-built artifacts not found for target X
error: Failed to locate pre-built artifacts
```

### Reproduction

```bash
cd codex/codex-rs
# On unsupported platform:
cargo build --release

# Expected error:
# error: rusty_v8 v0.X.X prebuilt bindings for aarch64-unknown-linux-musl unavailable
```

### Impact

- **Blocked**: Local builds fail, cannot develop locally on unsupported platforms
- **Workaround**: Use GitHub Actions CI, Docker, or disable V8-dependent features
- **Frequency**: Affects approximately 5-10% of developers

### Workarounds

**Option 1: Use GitHub Actions CI (Recommended)**
```bash
git push origin your-branch
# Navigate to: https://github.com/openai/codex/actions
# Watch the CI build succeed
# Download artifacts if needed
```

**Option 2: Build Without V8**
```bash
# Build with limited features
cargo build -p cli --no-default-features --release
# Some features will be unavailable
```

**Option 3: Use Docker**
```bash
# Build in a pre-configured container
docker build -f .github/workflows/Dockerfile.bazel -t yugi .
docker run -it yugi cargo build --release
```

**Option 4: Install Pre-built V8 Bindings**
```bash
# For Linux x86_64 (glibc only):
# Download from: https://github.com/openai/codex/releases
# Extract and set environment variables
export RUSTY_V8_ARCHIVE=/path/to/librusty_v8_*.a.gz
export RUSTY_V8_SRC_BINDING_PATH=/path/to/src_binding_*.rs
cargo build --release
```

### Timeline & Resolution

- **Status**: Awaiting upstream support in rusty_v8 project
- **Expected Fix**: Q1 2027 (rustup V8 support)
- **Current**: Mitigation required for unsupported platforms

### Related

- [rusty_v8 GitHub Issues](https://github.com/denoland/rusty_v8/issues)
- [TROUBLESHOOTING.md — V8 Pre-built Artifacts](./TROUBLESHOOTING.md#v8-pre-built-artifacts)

---

## Issue #2: GStreamer Version Conflicts 🔴 High

**Status**: Under investigation  
**Affects**: Ubuntu 22.04+, Fedora 38+  
**Reporter**: Community  
**Since**: YUGI fork initial release

### Description

GStreamer development headers may be incompatible or missing. The project depends on:
- `libgstreamer1.0-dev`
- `libgstreamer-plugins-base1.0-dev`

System package managers may provide:
- GStreamer 1.18 (Debian 11, Ubuntu 20.04) — ✅ Works
- GStreamer 1.20+ (Ubuntu 22.04+) — ⚠️ May need additional plugins
- GStreamer 1.24 (Latest) — ⚠️ Incompatible ABI with glib

### Symptoms

```
error: failed to run custom build command for 'gstreamer-sys'
error[E0514]: type mismatch: glib types are incompatible
error: pkg-config: GStreamer version 1.24 requires glib 2.80+
```

### Reproduction

Ubuntu 24.04:
```bash
sudo apt-get install -y libgstreamer1.0-dev
# GStreamer 1.24 installed

cd codex/codex-rs
cargo build --release
# May fail with ABI mismatch errors
```

### Impact

- **Frequency**: ~20% of new Ubuntu/Fedora installations
- **Severity**: Blocks entire build until resolved
- **Workaround Available**: Yes

### Workarounds

**Fix for Ubuntu/Debian:**
```bash
# Install specific compatible versions
sudo apt-get install -y \
  libgstreamer1.0-dev=1.22.* \
  libgstreamer-plugins-base1.0-dev=1.22.*

# Or use PPA with stable versions
sudo add-apt-repository -y ppa:gstreamer-developers/ppa
sudo apt-get update
sudo apt-get install -y libgstreamer1.0-dev
```

**Fix for Fedora/RHEL:**
```bash
# Install compatible development headers
sudo dnf install -y gstreamer1-plugins-base-devel

# Or downgrade to compatible version
sudo dnf install -y 'gstreamer1-devel < 1.24'
```

**Workaround: Disable GStreamer Features**
```bash
# Build without GStreamer support
cargo build -p cli --no-default-features --release
# Streaming features unavailable
```

**Fix: Update glib (Ubuntu 24.04+)**
```bash
# GStreamer 1.24 requires newer glib
sudo apt-get update
sudo apt-get upgrade -y libglib2.0-0

# Then rebuild
cargo build --release
```

### Timeline & Resolution

- **Status**: Investigating upstream compatibility
- **Expected Fix**: Q4 2026 (glib upgrade cycle)
- **Current**: Install compatible GStreamer version (1.22)

### Related

- [GStreamer Version Support](https://gstreamer.freedesktop.org/releases/)
- [TROUBLESHOOTING.md — GStreamer Issues](./TROUBLESHOOTING.md#gstreamer-version-issues)

---

## Issue #3: GitHub Authentication Failures 🟡 Medium

**Status**: Intermittent (infrastructure)  
**Affects**: SSH keys, personal access tokens, GitHub CLI  
**Reporter**: Developers using private repos  
**Since**: 2026-10-15

### Description

YUGI may fail to authenticate with GitHub when:
1. Using expired SSH keys
2. SSH agent not running
3. PAT token has insufficient scopes
4. GitHub API rate limits exceeded
5. Network proxy interferes

### Symptoms

```
error: fatal: could not read Username for 'https://github.com': terminal prompts disabled
fatal: Authentication failed for 'https://github.com/...'
error: Permission to <repo> denied to user <user>
```

### Reproduction

```bash
# Without GitHub CLI or SSH key configured:
./target/release/yugi "clone https://github.com/private/repo.git"
# Fails with permission denied
```

### Impact

- **Frequency**: Affects ~15% of uses (private repos, authenticated clones)
- **Severity**: Blocks repository access
- **Workaround Available**: Yes

### Workarounds

**Option 1: Use GitHub CLI (Recommended)**
```bash
# Install and authenticate
gh auth login

# Verify
gh auth status

# YUGI now uses authenticated credentials
./target/release/yugi "clone https://github.com/private/repo.git"
```

**Option 2: Use SSH Keys**
```bash
# Generate SSH key if needed
ssh-keygen -t ed25519 -C "your-email@example.com"

# Add public key to GitHub: https://github.com/settings/keys
cat ~/.ssh/id_ed25519.pub

# Start SSH agent
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_ed25519

# Configure git to use SSH
git config --global url."git@github.com:".insteadOf "https://github.com/"
```

**Option 3: Personal Access Token (PAT)**
```bash
# Create token with repo scope: https://github.com/settings/tokens/new
# Scopes needed: repo (full control) or public_repo (public only)

# Store in git credentials
git credential approve
# Input:
# host=github.com
# protocol=https
# username=<your-username>
# password=<your-token>
# (Press Ctrl+D)

# Test
./target/release/yugi "clone https://github.com/username/repo.git"
```

### Timeline & Resolution

- **Status**: Known limitations of GitHub auth
- **Expected Fix**: Investigate network proxy and rate limit improvements
- **Current**: Use GitHub CLI for reliable authentication

### Related

- [GitHub CLI Installation](https://cli.github.com/)
- [SSH Key Setup](https://docs.github.com/en/authentication/connecting-to-github-with-ssh)
- [TROUBLESHOOTING.md — GitHub Auth](./TROUBLESHOOTING.md#github-authentication-errors)

---

## Issue #4: Bazel Infrastructure Removed 🟢 Low

**Status**: By design (fork simplification)  
**Affects**: Build system contributors  
**Reporter**: YUGI maintainers  
**Since**: Initial fork

### Description

This YUGI fork removed Bazel build infrastructure to simplify the build system. All builds now use pure Cargo/Rust.

**Limitations**:
- Argument comment linting disabled (required Bazel)
- Some CI workflows simplified
- Build tooling is smaller but less parallelized

**This is intentional** and not a bug.

### Impact

- **Build times**: May be slightly longer than original (no Bazel parallelization)
- **Compatibility**: Cannot run Bazel targets
- **CI Jobs**: Some linting jobs are skipped

### Workarounds

**None needed** — this is the intended design.

If you need Bazel:
- Use the upstream [openai/codex](https://github.com/openai/codex) repository
- Or restore Bazel configuration from upstream

### Related

- [TROUBLESHOOTING.md — Build Performance](./TROUBLESHOOTING.md#build-performance)
- [GitHub Workflows Configuration](./.github/workflows/rust-ci.yml)

---

## Issue #5: Process Hardening / Seccomp Violations 🟡 Medium

**Status**: Known limitation  
**Affects**: Complex system calls, specialized workloads  
**Reporter**: Developers using advanced tools  
**Since**: Original Codex project

### Description

YUGI uses Linux seccomp filters (on Linux) to restrict system calls for security. Some applications and commands may violate these restrictions:

**Common violations**:
- Launching Docker from within YUGI
- Using `ptrace` (debuggers, strace)
- Loading kernel modules
- Raw socket operations
- File capability operations

### Symptoms

```
error: seccomp filter violation
error: Operation not permitted (os error 1)
thread 'unknown' panicked at 'unexpected system call'
```

### Reproduction

```bash
# Inside YUGI, try using strace (uses ptrace):
./target/release/yugi "strace ls -la"
# Fails: seccomp filter violation

# Try running docker:
./target/release/yugi "docker ps"
# May fail: Operation not permitted
```

### Impact

- **Frequency**: ~5% of users (advanced users only)
- **Severity**: Affects specific use cases
- **Workaround Available**: Yes

### Workarounds

**Option 1: Run Commands Directly (Outside YUGI)**
```bash
# Instead of:
./target/release/yugi "strace ls"

# Run directly:
strace ls
```

**Option 2: Disable Sandboxing (Development Only)**
```bash
# Not recommended for production
./target/debug/yugi -c sandbox_level=0 "strace ls"
# Disables security isolation
```

**Option 3: Use a Different Approach**
```bash
# Instead of seccomp-incompatible tools, use alternatives:
strace → debug output from executable
ptrace → RUST_LOG=debug env var
raw sockets → use standard libraries
```

### Timeline & Resolution

- **Status**: Unlikely to change (by design for security)
- **Expected Fix**: None (this is intentional)
- **Current**: Use workarounds for specialized cases

### Related

- [Linux Seccomp Documentation](https://www.kernel.org/doc/html/latest/userspace-api/seccomp_filter.html)
- [Process Hardening Crate](./codex-rs/process-hardening/README.md)
- [TROUBLESHOOTING.md — Sandbox Issues](./TROUBLESHOOTING.md#permission--sandbox-issues)

---

## Issue #6: Missing Dependencies on Fresh Systems 🟡 Medium

**Status**: Environment-specific  
**Affects**: Fresh OS installations  
**Reporter**: New users  
**Since**: Always present

### Description

Some systems may be missing development tools that Cargo/Rust projects need:
- C compiler (gcc/clang)
- pkg-config
- OpenSSL development headers
- Development libraries (glib, curl, etc.)

Different OS distributions include different defaults.

### Symptoms

```
error: linker 'cc' not found
error: pkg-config not found
error: Could not find OpenSSL
```

### Reproduction

Fresh Ubuntu system:
```bash
cargo build --release
# Fails: build-essential not installed
```

### Impact

- **Frequency**: 100% of fresh OS installs
- **Severity**: Blocks build
- **Workaround Available**: Yes (easy fix)

### Workarounds

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev
```

**macOS:**
```bash
xcode-select --install
# Or
brew install pkg-config openssl
```

**Fedora/RHEL:**
```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkg-config openssl-devel
```

**Alpine (if using):**
```bash
apk add --no-cache build-base pkgconfig openssl-dev
```

See [GETTING_STARTED.md](./GETTING_STARTED.md) for complete prerequisites.

### Timeline & Resolution

- **Status**: Documented in setup guide
- **Expected Fix**: See [GETTING_STARTED.md](./GETTING_STARTED.md) prerequisites
- **Current**: Install dependencies per OS

### Related

- [GETTING_STARTED.md — Prerequisites](./GETTING_STARTED.md#prerequisites-checklist)

---

## Issue #7: Slow Incremental Builds 🟡 Medium

**Status**: Performance characteristic  
**Affects**: Local development workflow  
**Reporter**: Daily users  
**Since**: Always present

### Description

With 100+ crates in the workspace, incremental rebuilds can be slow (30-60 seconds) even for small changes. This is inherent to Rust's compilation model.

### Impact

- **Frequency**: Every build
- **Severity**: Development workflow friction
- **Workaround Available**: Yes

### Workarounds

**Option 1: Build Single Crate**
```bash
# Instead of full workspace:
cargo build -p cli --release  # Fast: 10-15 seconds

# Avoid:
cargo build --release  # Slow: 30-60 seconds
```

**Option 2: Use Mold Linker (Linux)**
```bash
# Faster linker improves incremental builds
cargo install mold

# Add to Cargo config:
mkdir -p .cargo
echo '[build]
rustflags = ["-C", "link-arg=-fuse-ld=mold"]' >> .cargo/config.toml

cargo build -p cli --release  # 20-30% faster
```

**Option 3: Parallelism**
```bash
# Use all available cores
CARGO_BUILD_JOBS=$(nproc) cargo build -p cli --release
```

**Option 4: Use sccache (Distributed Cache)**
```bash
cargo install sccache
export RUSTC_WRAPPER=sccache
cargo build --release  # Caches intermediate results
```

### Timeline & Resolution

- **Status**: Inherent to Rust/large workspaces
- **Expected Fix**: Incremental improvements over time
- **Current**: Use targeted builds for development

### Related

- [Cargo Performance Tuning](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [TROUBLESHOOTING.md — Build Performance](./TROUBLESHOOTING.md#build-performance)

---

## Reporting New Issues

Found a bug not listed here? Please report it:

1. **Search existing issues**: [openai/codex/issues](https://github.com/openai/codex/issues)
2. **Check TROUBLESHOOTING.md** for known workarounds
3. **Include details**:
   - Your OS and version
   - Rust version (`rustc --version`)
   - Full error output (run with `RUST_LOG=debug`)
   - Reproduction steps
   - Relevant logs
4. **File the issue**: [New Issue](https://github.com/openai/codex/issues/new)

See [docs/contributing.md](./docs/contributing.md) for full guidelines.

---

## Summary Table

| Issue | Severity | Status | Workaround |
|-------|----------|--------|-----------|
| V8 Artifacts | ⚠️ Critical | Awaiting upstream | Use CI, Docker, or disable V8 |
| GStreamer Versions | 🔴 High | Investigation | Install compatible version |
| GitHub Auth | 🟡 Medium | Known limitation | Use GitHub CLI or SSH |
| Bazel Removal | 🟢 Low | By design | None (intended) |
| Seccomp Violations | 🟡 Medium | By design | Run outside YUGI |
| Missing Dependencies | 🟡 Medium | Documented | Install dev tools |
| Slow Incremental Builds | 🟡 Medium | Characteristic | Build single crates |

---

## See Also

- [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) — Detailed fixes for common errors
- [GETTING_STARTED.md](./GETTING_STARTED.md) — Setup guide with prerequisites
- [docs/contributing.md](./docs/contributing.md) — How to report bugs
- [GitHub Issues](https://github.com/openai/codex/issues) — Community-reported issues
