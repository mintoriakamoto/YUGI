# GitHub Actions Workflows Guide

This document describes the CI/CD workflows that run automatically on pull requests and can be triggered manually. All workflows follow the principle of fail-fast quality enforcement.

---

## Workflow Overview

| Workflow | Trigger | Purpose | Status |
|----------|---------|---------|--------|
| `build.yml` | PR on codex-rs changes | Build all targets and dependencies | Required ✅ |
| `quality.yml` | PR on codex-rs changes | Lint, audit, and code quality checks | Required ✅ |
| `testing.yml` | PR on codex-rs/sdk/scripts changes | Run full test suite with coverage | Required ✅ |

---

## build.yml

**Purpose**: Verifies that the codebase compiles successfully and all dependencies resolve correctly.

### When it runs
- On pull requests that modify anything in `codex-rs/`
- On workflow file changes (`.github/workflows/build.yml`)
- Manually via `workflow_dispatch`

### What it does
1. Checks out the repository
2. Sets up CI environment (via `.github/actions/setup-ci`)
3. Installs Rust toolchain (1.99.1)
4. Caches Cargo registry and build artifacts
5. Runs `cargo check --all-targets --all-features`
6. Runs `cargo build --all-targets --all-features`
7. Generates a build report in the job summary
8. Verifies clean worktree

### Success criteria
- All targets compile without warnings or errors
- No uncommitted changes introduced by build
- All dependencies resolve correctly

### Debugging failures

**Symptoms**: Build fails with compilation errors

**Debugging steps**:
1. Check error message in the workflow run logs
2. Run locally: `cd codex-rs && cargo build --all-targets --all-features`
3. If local build succeeds, check for Rust version mismatch: `rustc --version` (should match 1.99.1)
4. Clear cache and rebuild: `cargo clean && cargo build`
5. Check dependency locks: `cd codex-rs && cargo update`

**Common issues**:
- **"Outdated dependencies"**: The Cargo.lock file may be out of sync. Run `cargo update` locally and commit.
- **"Missing system libraries"**: Ensure all system dependencies from `.github/actions/setup-ci` are installed.
- **"MSRV failure"**: Code may not compile on Rust 1.99.1. Check `codex-rs/rust-toolchain` for expected version.

### Skipping locally (and why not to)

You can skip the build check locally by not running cargo build, but **you should not**:

```bash
# Don't do this — you'll find issues in CI instead of locally
# cargo build --no-default-features

# Do this instead
cd codex-rs
cargo build --all-targets --all-features
```

Why: Building locally catches issues immediately, before pushing. Skipping means broken builds get to CI.

---

## quality.yml

**Purpose**: Enforces code quality standards via static analysis and security audits.

### When it runs
- On pull requests that modify anything in `codex-rs/`
- On changes to quality configuration files (`ruff.toml`)
- On workflow file changes (`.github/workflows/quality.yml`)
- Manually via `workflow_dispatch`

### What it does

#### Clippy (Rust linter)
1. Installs Rust toolchain with clippy component
2. Runs: `cargo clippy --all-targets -- -D warnings`
   - `-D warnings` treats warnings as errors (fail-fast)
   - Checks all targets (binaries, tests, examples, benches)
3. Reports all clippy violations as errors

#### Cargo Deny (Dependency auditor)
1. Installs Rust toolchain
2. Runs: Cargo Deny with `EmbarkStudios/cargo-deny-action@v2`
   - Scans `Cargo.lock` for known security vulnerabilities
   - Checks license compliance
   - Prevents use of unmaintained crates

#### Ruff (Python linter)
1. Sets up Python 3.11
2. Installs ruff
3. Runs: `ruff check . --config ruff.toml`
   - Checks all Python code (except sdk/python and codex-rs/vendor per `ruff.toml`)
   - Enforces consistent style and catches common errors

#### Aggregator job
- Summarizes all three checks
- Fails if any check fails
- Posts summary to PR

### Success criteria
- No clippy warnings (all treated as errors)
- No known vulnerabilities in dependencies
- No banned licenses detected
- Python code passes ruff checks

### Debugging failures

**Symptoms**: Clippy reports warnings

**Debugging steps**:
1. Read the clippy warning message carefully
2. Run locally: `cd codex-rs && cargo clippy --all-targets -- -D warnings`
3. Fix the issue according to clippy's suggestion (usually provided)
4. If unsure, check clippy docs: `rustup doc --edition 2021` → Clippy

**Symptoms**: Cargo Deny finds vulnerabilities

**Debugging steps**:
1. Check the denial reason in the workflow output
2. Run locally: `cd codex-rs && cargo tree` (to see dependency tree)
3. If vulnerability: update the affected crate or contact maintainers
4. If license issue: check `Cargo.toml` for alternative crate or request license exception
5. View deny config: `.github/deny.toml` (if exists) or Cargo.toml `[package.metadata]`

**Symptoms**: Ruff finds style issues

**Debugging steps**:
1. Read ruff's error message (usually self-explanatory)
2. Run locally: `ruff check . --config ruff.toml`
3. Auto-fix many issues: `ruff check . --fix`
4. If auto-fix doesn't work, manually adjust per ruff's guidance
5. Re-run: `ruff check . --config ruff.toml`

### Skipping locally (and why not to)

You can skip quality checks locally, but **you should not**:

```bash
# Don't do this — you'll fail in CI instead
# cargo build --all-features  # Without clippy

# Do this instead
cd codex-rs
cargo clippy --all-targets -- -D warnings
cargo deny check
ruff check ..
```

Why: Quality issues compound over time. Catching them locally keeps the codebase maintainable.

---

## testing.yml

**Purpose**: Ensures all tests pass and tracks code coverage.

### When it runs
- On pull requests that modify:
  - `codex-rs/**` (Rust code)
  - `sdk/python/**` (Python SDK)
  - `scripts/**` (Test scripts)
- On workflow file changes (`.github/workflows/testing.yml`)
- Manually via `workflow_dispatch`

### What it does

#### Rust tests
1. Checks out repository
2. Sets up CI environment
3. Installs Rust toolchain (1.99.1)
4. Installs cargo-nextest (0.9.72) — parallel test runner
5. Caches build artifacts
6. Runs: `cargo nextest run --no-fail-fast`
   - `--no-fail-fast`: Runs all tests even if one fails (gives complete picture)
   - Parallelizes test execution for speed
   - Configured with `NEXTEST_PROFILE=ci`
7. Uploads nextest results as artifacts

#### Python tests
1. Sets up Python 3.11
2. Installs pytest and pytest-cov
3. Runs: `python -m pytest scripts/ sdk/python/tests/ -v --tb=short`
   - `-v`: Verbose output (shows each test)
   - `--tb=short`: Short traceback format
4. Uploads coverage reports as artifacts

#### Test markers

The test suite supports markers to conditionally run tests:

| Marker | Meaning | CI Behavior |
|--------|---------|-------------|
| `#[cfg(test)]` | Standard Rust test | Always runs |
| `#[tokio::test]` | Async test | Always runs |
| `@pytest.mark.skip` | Python test (skipped) | Skipped always |
| `@pytest.mark.skipif` | Conditional skip | Skipped if condition met |

**Note**: GPU tests and V8-specific tests should use platform-specific skipif markers.

#### Aggregator job
- Summarizes Rust and Python test results
- Fails if any test fails
- Links to uploaded artifacts for debugging

### Success criteria
- All Rust tests pass (via cargo nextest)
- All Python tests pass (via pytest)
- No test output indicates test infrastructure issues
- Code coverage meets project standards (if configured)

### Debugging failures

**Symptoms**: Individual test fails

**Debugging steps**:
1. Read the test output in the workflow logs
2. Run the test locally:
   ```bash
   # For Rust tests
   cd codex-rs
   RUST_MIN_STACK=8388608 cargo nextest run <test-name>
   
   # For Python tests
   python -m pytest scripts/ sdk/python/tests/ -k <test-name> -v
   ```
3. Debug the failing test (set breakpoints, add logging)
4. Verify fix with full test suite locally
5. Push and re-run workflow

**Symptoms**: Test suite times out

**Debugging steps**:
1. Check which test is slow in the logs
2. Run that test locally with timing: `cargo nextest run <test-name> -- --nocapture`
3. Look for infinite loops, deadlocks, or external service calls
4. Mark slow tests with `#[ignore]` if they're meant to be opt-in
5. Consider splitting large tests into smaller units

**Symptoms**: Flaky tests (pass/fail unpredictably)

**Debugging steps**:
1. Re-run the test multiple times locally to reproduce
2. Check for race conditions, timing assumptions, or external state
3. Use `RUST_LOG=debug` to enable logging
4. Consider marking flaky tests temporarily with `#[ignore]` while fixing
5. Document root cause and fix

### Skipping locally (and why not to)

You can skip tests locally, but **you should not**:

```bash
# Don't do this — you'll fail in CI instead
# cargo build --skip-tests

# Do this instead
cd codex-rs
RUST_MIN_STACK=8388608 cargo nextest run
python -m pytest scripts/ sdk/python/tests/
```

Why: Tests catch regressions early. Skipping them locally means bugs reach CI or production.

---

## Workflow Triggers and Status Checks

### Branch Protection Rules

To enforce these workflows, add the following as **required status checks** in repository branch protection:

1. **build.yml**
   - Required status: `build / Build`
   - Do not allow bypass

2. **quality.yml**
   - Required status: `quality / Quality gates (required)`
   - Do not allow bypass

3. **testing.yml**
   - Required status: `testing / Test results (required)`
   - Do not allow bypass

### Manually triggering workflows

All workflows can be triggered manually via GitHub UI:

```bash
# Trigger via CLI (requires gh)
gh workflow run build.yml
gh workflow run quality.yml
gh workflow run testing.yml
```

---

## Debugging Workflow Failures

### General steps

1. **Read the workflow output**: Click on the failing job in the GitHub UI
2. **Check the logs**: Expand each step to see what failed
3. **Look for error context**: Workflow logs usually indicate the exact problem
4. **Run locally**: Reproduce the failure in your local environment first
5. **Check environment**: Ensure your local environment matches the CI environment (OS, Rust version, etc.)

### Common issues

| Issue | Cause | Fix |
|-------|-------|-----|
| "Cache miss on every run" | Cargo.lock hash changes frequently | Commit Cargo.lock changes |
| "Clippy passes locally, fails in CI" | Rust version mismatch | Check `dtolnay/rust-toolchain` version in workflow |
| "Test passes once, fails later" | Flaky test or race condition | Run test multiple times locally, add synchronization |
| "Workflow timeout" | Tests taking too long | Profile slow tests, split into smaller units |

### Viewing artifacts

Test results and logs are stored as artifacts. To download:

1. Go to the workflow run in GitHub UI
2. Scroll to "Artifacts" section
3. Download desired artifact (e.g., `rust-test-results`)
4. Examine results locally

---

## Maintenance

### Updating Rust version

To update the Rust toolchain version across all workflows:

1. Edit `.github/workflows/build.yml`
2. Edit `.github/workflows/quality.yml`
3. Edit `.github/workflows/testing.yml`
4. Change `dtolnay/rust-toolchain@<hash>` version number
5. Also update `codex-rs/rust-toolchain` file (if exists)
6. Test locally with new version
7. Commit and push

### Adding new tests

1. Add test to `codex-rs/src/**/*.rs` or `scripts/` or `sdk/python/tests/`
2. Ensure test has appropriate markers if conditional
3. Run locally: `just test` (Rust) or `pytest` (Python)
4. Push and let CI validate
5. If test fails in CI but passes locally, check environment-specific issues

### Adding new quality checks

1. Create new GitHub Actions workflow in `.github/workflows/`
2. Add to the overview table in this file
3. Consider adding as required status check in branch protection
4. Document in this file with the same format

---

## See Also

- [GitHub Actions Workflows](https://docs.github.com/en/actions/using-workflows)
- [cargo-nextest documentation](https://nexte.st/)
- [Clippy lints](https://rust-lang.github.io/rust-clippy/)
- [Cargo deny](https://embarkstudios.github.io/cargo-deny/)
- [Ruff documentation](https://docs.astral.sh/ruff/)
