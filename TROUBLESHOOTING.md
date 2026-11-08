# Troubleshooting Guide

This guide covers the most common issues when building, running, or developing YUGI locally.

## Quick Diagnosis: What Type of Issue?

```
START HERE: What's your problem?

├─ Build fails? 
│  ├─ V8 artifacts error? → See "V8 Pre-built Artifacts"
│  ├─ GStreamer not found? → See "GStreamer Version Issues"
│  ├─ Linking errors? → See "Compiler & Linker Issues"
│  └─ Other compilation? → See "General Compilation Issues"
│
├─ Runtime error?
│  ├─ Permission denied? → See "Permission & Sandbox Issues"
│  ├─ File not found? → See "File & Path Issues"
│  └─ Authentication failed? → See "GitHub Authentication"
│
├─ Testing fails?
│  ├─ Tests panic? → See "Test Failures"
│  └─ Nextest issues? → See "Test Runner Issues"
│
└─ Performance?
   ├─ Build is slow? → See "Build Performance"
   └─ Runtime is slow? → See "Runtime Performance"
```

---

## Build Issues

### V8 Pre-built Artifacts

**Symptom:**
```
error: V8 pre-built artifacts not found for target X
error: rusty_v8 v0.X.X prebuilt bindings unavailable
```

**Root Cause:**  
YUGI uses pre-built rusty_v8 binaries from GitHub releases. These are only built for specific platform/architecture combinations. If your target is not supported, you must use an alternative.

**Workaround — Option 1: Use GitHub Actions CI**
```bash
# Push your changes and let CI build for you
git push origin your-branch
# Check GitHub Actions: https://github.com/openai/codex/actions
```

**Workaround — Option 2: Build Without V8 Features**
```bash
# Disable features that require V8
cargo build -p cli --no-default-features --release
```

**Workaround — Option 3: Use Docker**
```bash
# Use the provided Dockerfile to build in a consistent environment
docker build -f .github/workflows/Dockerfile.bazel -t yugi .
docker run -it yugi cargo build --release
```

**Impact:**  
- Local builds may fail on unsupported platforms
- CI always succeeds (it uses pre-built artifacts)
- Workarounds allow development but may have limited functionality

**Timeline:**  
Awaiting upstream rusty_v8 support for additional platforms.

---

### GStreamer Version Issues

**Symptom:**
```
error: failed to run custom build command for `gstreamer-sys v...`
error: pkg-config not found or GStreamer development files not installed
```

**Root Cause:**  
The project depends on GStreamer for media/streaming features. Your system either lacks GStreamer or has an incompatible version.

**Fix — Ubuntu/Debian:**
```bash
# Install GStreamer development libraries
sudo apt-get update
sudo apt-get install -y \
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-good1.0-dev \
  pkg-config

# Verify installation
pkg-config --modversion gstreamer-1.0
```

**Expected**: Output like `1.20.x` or `1.24.x`

**Fix — macOS:**
```bash
# Install GStreamer via Homebrew
brew install gstreamer gstreamer-plugins-base

# Verify installation
pkg-config --modversion gstreamer-1.0
```

**Fix — Windows (WSL2):**
Use the Ubuntu/Debian fix above within your WSL2 environment.

**Workaround — If GStreamer Cannot Be Installed:**
```bash
# Disable features that require GStreamer
cargo build -p cli --no-default-features --release
```

**Impact:**  
- Build fails or hangs waiting for GStreamer
- Some media/streaming features unavailable if disabled

---

### Compiler & Linker Issues

**Symptom:**
```
error: linker 'cc' not found
error: could not compile `some-crate`
error: undefined reference to '_something'
```

**Root Cause:**  
Missing C compiler or standard development tools.

**Fix — Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev
```

**Fix — macOS:**
```bash
# Install Xcode Command Line Tools
xcode-select --install

# If already installed, verify:
xcode-select -p  # Should print /Applications/Xcode.app/...
```

**Fix — Fedora/RHEL:**
```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkg-config openssl-devel
```

**Verify Installation:**
```bash
gcc --version
cc --version
pkg-config --version
```

---

### General Compilation Issues

**Symptom:**
```
error: expected one of `.`, `?`, ...]
error: failed to parse manifest
```

**Typical Causes:**
1. Outdated Rust toolchain
2. Corrupted dependency cache
3. Incompatible dependency versions

**Fix — Update Toolchain:**
```bash
rustup update
rustup component add rustfmt clippy
```

**Fix — Clean Build:**
```bash
# Remove build artifacts and cache
cargo clean

# Rebuild from scratch
cargo build --release
```

**Fix — Check Dependency Versions:**
```bash
# Verify Cargo.lock is consistent
cargo update --dry-run

# If issues persist, regenerate lock file
rm Cargo.lock
cargo update
cargo build --release
```

---

## Runtime Issues

### Permission & Sandbox Issues

**Symptom:**
```
error: permission denied (os error 13)
error: sandbox violation
error: seccomp filter violation
```

**Root Cause:**  
YUGI uses process hardening and sandboxing to restrict command execution. Your command may violate these restrictions.

**Diagnostic:**
```bash
# Check sandbox logs
RUST_LOG=debug ./target/debug/yugi "your command"

# Save detailed logs
./target/debug/yugi -c log_dir=./logs "your command"
tail -f ./logs/*.log
```

**Fix — Check Permissions:**
```bash
# Verify you have permission to execute the command
ls -la /path/to/executable

# Check if process-hardening applies
grep -r "seccomp\|apparmor\|selinux" ~/.codex/logs/
```

**Fix — Escalate Permissions (Careful!):**
```bash
# Run with sudo only if necessary
sudo ./target/release/yugi "your command"
```

**Workaround — Disable Sandboxing (Development Only):**
Add to your config:
```bash
./target/debug/yugi -c sandbox_level=0 "your command"
```

**Impact:**  
- Commands may fail unexpectedly
- Security features are disabled if you escalate permissions

---

### File & Path Issues

**Symptom:**
```
error: file not found
error: no such file or directory
error: cannot resolve path
```

**Root Cause:**  
Paths are relative to the YUGI working directory, not your shell directory.

**Diagnostic:**
```bash
# Check YUGI's working directory
./target/debug/yugi -c log_dir=./logs "pwd"

# Check file availability
./target/debug/yugi "ls -la /path/to/file"
```

**Fix — Use Absolute Paths:**
```bash
# Instead of relative paths
./target/release/yugi "cat ./README.md"  # May fail

# Use absolute paths
./target/release/yugi "cat $(pwd)/README.md"  # Works
```

**Fix — Set YUGI_HOME:**
```bash
# Explicitly set working directory
YUGI_HOME=$PWD ./target/release/yugi "your command"
```

---

### GitHub Authentication Errors

**Symptom:**
```
error: 401 Unauthorized
error: fatal: could not read Username
error: Permission to <repo> denied to user <user>
```

**Root Cause:**  
YUGI cannot authenticate with GitHub for cloning or pulling repositories.

**Fix — Configure Git Credentials:**
```bash
# Use GitHub CLI for authentication (recommended)
gh auth login
# Follow the prompts

# Verify authentication
gh auth status
```

**Fix — Use SSH Keys:**
```bash
# Generate SSH key if needed
ssh-keygen -t ed25519 -C "your-email@example.com"

# Add to GitHub: https://github.com/settings/keys
cat ~/.ssh/id_ed25519.pub

# Configure Git to use SSH
git config --global url."git@github.com:".insteadOf "https://github.com/"
```

**Fix — HTTPS with Personal Access Token:**
```bash
# Create token: https://github.com/settings/tokens
# Store securely in git credentials
git credential approve
host=github.com
protocol=https
username=<your-username>
password=<your-token>
# (type Ctrl+D)

# Test
git clone https://github.com/openai/codex.git
```

---

## Testing Issues

### Test Failures

**Symptom:**
```
test result: FAILED. failures: <N>
thread 'tests::...' panicked at '...'
```

**Diagnostic:**
```bash
# Run a single failing test with output
cargo test <test-name> -- --nocapture --exact

# Run tests for one crate only
cargo test -p <crate-name>

# Run with debug logging
RUST_LOG=debug cargo test -- --nocapture
```

**Common Fixes:**
```bash
# Clean and rebuild
cargo clean
cargo test

# Update dependencies
cargo update
cargo test

# Check if test uses file paths
# → May need to run from specific directory
cd codex/codex-rs
cargo test
```

---

### Test Runner Issues

**Symptom:**
```
error: nextest not found
error: failed to run nextest
```

**Fix:**
```bash
# Install nextest
cargo install --locked cargo-nextest

# Verify
cargo nextest --version

# Run tests with nextest
cargo nextest run

# Or use standard test runner
cargo test
```

---

## Performance Issues

### Build Performance

**Symptom:**  
Builds take > 30 minutes, or disk usage is excessive.

**Cause:**  
Large dependency tree, multiple feature combinations, or debug builds.

**Fix — Build in Release Mode:**
```bash
# Faster (optimized) builds
cargo build --release  # Slower first time, faster incremental

# Debug mode is faster first build, slow incremental
cargo build  # Skip for repeated builds
```

**Fix — Build Single Crate:**
```bash
# Instead of entire workspace
cargo build -p cli --release
```

**Fix — Avoid --all-features:**
```bash
# Don't build all feature combinations locally
cargo build --release  # Uses default features

# Avoid:
cargo build --all-features --release  # Very slow
```

**Fix — Use Incremental Builds:**
```bash
# Incremental rebuilds after small changes
touch codex-rs/cli/src/main.rs
cargo build -p cli --release  # Much faster
```

**Fix — Increase Build Parallelism:**
```bash
# Use all available CPU cores
cargo build -j $(nproc) --release
```

### Runtime Performance

**Symptom:**  
YUGI runs slowly, consumes excessive memory, or hangs.

**Diagnostic:**
```bash
# Monitor resource usage
time ./target/release/yugi "your command"

# Check memory usage
/usr/bin/time -v ./target/release/yugi "your command"

# Profile execution
RUST_BACKTRACE=1 ./target/debug/yugi "your command"
```

**Common Fixes:**
```bash
# Run release build (not debug)
./target/release/yugi "command"  # Fast

# Avoid:
./target/debug/yugi "command"  # Slow

# Disable logging for speed
./target/release/yugi "command"  # No RUST_LOG set

# Increase available memory if OOM
free -h  # Check available RAM
```

---

## Still Stuck?

If you cannot find your issue here:

1. **Check logs**: `RUST_LOG=debug cargo build` captures detailed output
2. **Search GitHub Issues**: [openai/codex/issues](https://github.com/openai/codex/issues)
3. **Check KNOWN_ISSUES.md**: Detailed tracking of ongoing problems
4. **File a new issue**: Include logs, platform details, and reproduction steps

---

## Decision Tree: Local vs. CI

Use this to decide whether to troubleshoot locally or use CI:

```
Can't build locally?
├─ V8 artifacts? → Use CI build
├─ GStreamer? → Install or use CI
├─ Compiler? → Install toolchain
└─ Other? → Debug locally or check KNOWN_ISSUES.md

Ready to commit?
├─ Changes are small? → Test locally with `cargo test -p <crate>`
├─ Changes are complex? → Push and let CI validate
└─ Not sure? → Push and see CI results
```

---

## See Also

- [GETTING_STARTED.md](./GETTING_STARTED.md) — Setup guide
- [KNOWN_ISSUES.md](./KNOWN_ISSUES.md) — Detailed issue tracking
- [docs/install.md](./docs/install.md) — Installation instructions
- [docs/contributing.md](./docs/contributing.md) — How to report issues
