# Getting Started with YUGI

**Target audience**: New developers who want to build and run YUGI locally.  
**Time estimate**: 15 minutes (download and build times vary by system)

## Prerequisites Checklist

Before you begin, verify you have:

- [ ] **Operating System**: macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 with WSL2
- [ ] **Memory**: 4 GB RAM available (8 GB recommended for faster builds)
- [ ] **Git**: Version 2.23 or later (`git --version`)
- [ ] **Python**: Version 3.9+ (`python3 --version`)
- [ ] **Development tools**:
  - Linux: `build-essential`, `pkg-config` (install with `sudo apt-get install build-essential pkg-config`)
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Windows (WSL2): Same as Linux

## Step 1: Install Rust (5 minutes)

```bash
# Download and install rustup (Rust toolchain installer)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Load the new Rust environment
source "$HOME/.cargo/env"

# Verify installation
rustc --version
cargo --version
```

**Expected output**:
```
rustc 1.99.x (stable)
cargo 1.99.x
```

## Step 2: Clone the Repository (2 minutes)

```bash
# Clone the YUGI repository
git clone https://github.com/openai/codex.git
cd codex/codex-rs

# Verify the repository structure
ls -la | head
```

**Expected**: You should see `Cargo.toml` in the current directory.

## Step 3: Install Development Tools (3 minutes)

```bash
# Install just (command runner for Rust projects)
cargo install --locked just

# Install cargo-nextest (test runner)
cargo install --locked cargo-nextest

# Verify installations
just --version
cargo nextest --version
```

## Step 4: Build YUGI (5-10 minutes)

```bash
# Navigate to the workspace root
cd codex/codex-rs  # (if not already there)

# Build the project in release mode
cargo build --release

# This will download dependencies and compile all crates
# Note: First build takes longer; subsequent builds are faster
```

**Expected**:
- Build completes with no errors
- Binary appears at `./target/release/yugi`

**If build fails**: Jump to [Validation & Troubleshooting](#validation--troubleshooting) below.

## Step 5: Validate Your Installation

### Test 1: Run YUGI Basic Command

```bash
# Run with a simple command
./target/release/yugi "what is 2 + 2?"
```

**Expected**: YUGI responds with a result.

### Test 2: Run Local Tests

```bash
# Run the test suite for the CLI crate
just test -p cli

# Full test suite (slower)
just test
```

**Expected**: All tests pass with summary output like:
```
    Summary  [   X.XXXs] 123 tests run: 123 passed
```

### Test 3: Check Code Format

```bash
# Verify code follows project style
cargo fmt -- --check

# Apply formatting if needed
cargo fmt
```

**Expected**: No errors; if formatting is needed, apply it with `cargo fmt`.

### Test 4: Lint with Clippy

```bash
# Run Rust linter
cargo clippy -p cli
```

**Expected**: No warnings (or informational warnings only, no errors).

## Development Workflow

### Making Changes

```bash
# Edit a file in the project
# Example: edit codex-rs/cli/src/main.rs

# Format your changes
cargo fmt

# Check for issues
cargo clippy

# Test your changes
cargo test -p <crate-name>

# Build release binary
cargo build --release
```

### Useful Commands

```bash
# View available just recipes
just --list

# Format all code
just fmt

# Run clippy on a specific crate
just fix -p cli

# Build documentation
cargo doc --no-deps --open
```

### Checking Logs & Debug Output

```bash
# Run with verbose logging
RUST_LOG=debug ./target/debug/yugi "your command here"

# Save logs to file
./target/debug/yugi -c log_dir=./logs "your command here"
tail -f ./logs/codex-tui.log
```

## Validation & Troubleshooting

### Issue: Build fails with "V8 artifacts not found"

**Cause**: Pre-built rusty_v8 bindings are not available for your platform.

**Workaround**: 
- Use CI builds via GitHub Actions
- Or build with: `cargo build --release -p cli`
- See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) for details

### Issue: "GStreamer not found"

**Cause**: Missing GStreamer development libraries.

**Fix (Ubuntu/Debian)**:
```bash
sudo apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

**Fix (macOS)**:
```bash
brew install gstreamer
```

### Issue: "cargo not found" or other tool errors

**Cause**: Rust toolchain not properly loaded.

**Fix**:
```bash
source "$HOME/.cargo/env"
rustc --version  # Should succeed
```

### Issue: Tests fail with permission errors

**Cause**: Insufficient permissions or sandboxing issues.

**Fix**:
```bash
# Run with appropriate permissions
just test

# Or for a single crate
cargo test -p cli -- --nocapture
```

## Next Steps

1. **Read the code**: Start with `codex-rs/cli/src/main.rs`
2. **Explore architecture**: Check out [Project Structure](./README.md#project-structure)
3. **Build something**: Try modifying a crate and testing your changes
4. **Review documentation**: See [README.md](./README.md) for full feature list
5. **File issues**: Found a bug? Report it on [GitHub Issues](https://github.com/openai/codex/issues)

## Common First Errors & Fixes

| Error | Cause | Fix |
|-------|-------|-----|
| `cargo: command not found` | Rust not installed or not in PATH | Run `source "$HOME/.cargo/env"` |
| `git: command not found` | Git not installed | Install git via package manager |
| `error: linker 'cc' not found` | Missing C compiler | Install `build-essential` (Linux) or Xcode Tools (macOS) |
| `V8 artifacts not available` | Platform-specific build issue | Use CI or consult [KNOWN_ISSUES.md](./KNOWN_ISSUES.md) |
| `permission denied` | Sandbox/permission issue | Run with `sudo` or check process-hardening logs |

## Support Resources

- **Quick help**: [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) — Common issues and fixes
- **Known issues**: [KNOWN_ISSUES.md](./KNOWN_ISSUES.md) — Detailed tracking
- **Build instructions**: [docs/install.md](./docs/install.md) — System-specific details
- **Contributing**: [docs/contributing.md](./docs/contributing.md) — How to report bugs
- **Repository**: [openai/codex](https://github.com/openai/codex) — Main project

## Success!

Once you've completed all five steps and passed all four validation tests, you have a working YUGI development environment. You're ready to:

- Build and test changes locally
- Run YUGI as a local agent
- Contribute bug reports and analysis
- Explore the codebase architecture

Happy coding!
