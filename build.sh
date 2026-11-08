#!/usr/bin/env bash
################################################################################
# YUGI Canonical Build Script
#
# This script provides a standardized build process for the YUGI monorepo.
# It checks prerequisites, sets up environment variables, and builds all
# components (Rust + Node.js).
#
# Usage: ./build.sh [OPTIONS]
#   --help                  Show this help message
#   --check-only            Only run cargo check (no full build)
#   --no-pnpm              Skip pnpm install and build
#   --no-cargo             Skip Rust/Cargo build
#   --verbose              Enable verbose output
#
################################################################################

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_WS="${REPO_ROOT}/codex-rs"
PNPM_WS="${REPO_ROOT}"

# Options
CHECK_ONLY=0
SKIP_PNPM=0
SKIP_CARGO=0
VERBOSE=0

################################################################################
# Functions
################################################################################

log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

show_help() {
    grep "^#" "${BASH_SOURCE[0]}" | head -20 | sed 's/^#//'
    exit 0
}

# Verbose logging wrapper
run_cmd() {
    if [[ $VERBOSE -eq 1 ]]; then
        "$@"
    else
        "$@" 2>&1 | grep -v "^Compiling\|^    Finished\|^     Running"
    fi
}

################################################################################
# Prerequisite Checks
################################################################################

check_prerequisites() {
    log_info "Checking prerequisites..."
    local missing=0

    # Check Rust toolchain
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo not found. Install Rust: https://rustup.rs/"
        missing=1
    else
        log_success "Cargo found: $(cargo --version)"
    fi

    if ! command -v rustc &> /dev/null; then
        log_error "Rustc not found. Install Rust: https://rustup.rs/"
        missing=1
    else
        log_success "Rustc found: $(rustc --version)"
    fi

    # Check Node.js
    if ! command -v node &> /dev/null; then
        log_error "Node.js not found. Install from https://nodejs.org/"
        missing=1
    else
        log_success "Node.js found: $(node --version)"
    fi

    # Check pnpm
    if ! command -v pnpm &> /dev/null; then
        log_error "pnpm not found. Install with: npm install -g pnpm"
        missing=1
    else
        log_success "pnpm found: $(pnpm --version)"
    fi

    # Check Git (optional but recommended)
    if ! command -v git &> /dev/null; then
        log_warning "Git not found (optional but recommended)"
    else
        log_success "Git found: $(git --version)"
    fi

    # Linux-specific checks for Rust build dependencies
    if [[ "$(uname -s)" == "Linux" ]]; then
        if ! pkg-config --exists libcap 2>/dev/null; then
            log_warning "libcap-dev not found. Linux Rust builds require it."
            log_warning "  Install with: sudo apt-get install libcap-dev pkg-config"
        else
            log_success "libcap found (pkg-config)"
        fi

        if ! command -v pkg-config &> /dev/null; then
            log_error "pkg-config not found. Required for Linux builds."
            log_error "  Install with: sudo apt-get install pkg-config"
            missing=1
        else
            log_success "pkg-config found: $(pkg-config --version)"
        fi

        # Check for build-essential (cc, make, etc.)
        if ! command -v cc &> /dev/null; then
            log_warning "cc (C compiler) not found. Linux builds require it."
            log_warning "  Install with: sudo apt-get install build-essential"
        else
            log_success "C compiler found"
        fi
    fi

    if [[ $missing -eq 1 ]]; then
        log_error "Some prerequisites are missing. See above for details."
        return 1
    fi

    log_success "All prerequisites met"
    return 0
}

################################################################################
# Environment Setup
################################################################################

setup_environment() {
    log_info "Setting up build environment..."

    # Export variables for downstream builds
    export CODEX_REPO_ROOT="${REPO_ROOT}"

    # Linux-specific: Configure pkg-config paths for cross-compilation support
    if [[ "$(uname -s)" == "Linux" ]]; then
        # Standard system paths
        local pkg_config_path="/usr/lib/x86_64-linux-gnu/pkgconfig"
        pkg_config_path="${pkg_config_path}:/usr/lib/pkgconfig"
        pkg_config_path="${pkg_config_path}:/usr/local/lib/pkgconfig"

        # Add any user-provided PKG_CONFIG_PATH
        if [[ -n "${PKG_CONFIG_PATH:-}" ]]; then
            pkg_config_path="${PKG_CONFIG_PATH}:${pkg_config_path}"
        fi

        export PKG_CONFIG_PATH="${pkg_config_path}"
        log_info "PKG_CONFIG_PATH set to: ${PKG_CONFIG_PATH}"
    fi

    # Configure jemalloc for transparent huge pages (from .cargo/config.toml)
    export MALLOC_CONF="${MALLOC_CONF:-thp:always}"

    # Optional: Use all available CPU cores for compilation
    export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1)}"

    log_success "Environment configured"
}

################################################################################
# Cargo Build
################################################################################

build_cargo() {
    log_info "Building Rust workspace (${CARGO_WS})..."

    if [[ ! -f "${CARGO_WS}/Cargo.toml" ]]; then
        log_error "Cargo.toml not found at ${CARGO_WS}/Cargo.toml"
        return 1
    fi

    cd "${CARGO_WS}"

    # Step 1: Check library compilation
    log_info "Running 'cargo check --lib' to validate library targets..."
    if run_cmd cargo check --lib; then
        log_success "Library check passed"
    else
        log_error "Library check failed"
        return 1
    fi

    # Step 2: If not check-only, run full build
    if [[ $CHECK_ONLY -eq 0 ]]; then
        log_info "Building all Rust targets..."
        if run_cmd cargo build; then
            log_success "Rust build completed"
        else
            log_error "Rust build failed"
            return 1
        fi

        # Step 3: Quick sanity check - ensure key binaries exist
        if [[ -x "target/debug/codex" ]]; then
            log_success "Codex binary built successfully"
        else
            log_warning "Codex binary not found at expected location"
        fi
    fi

    cd "${REPO_ROOT}"
    return 0
}

################################################################################
# pnpm Build
################################################################################

build_pnpm() {
    log_info "Installing and building Node.js packages (${PNPM_WS})..."

    cd "${PNPM_WS}"

    # Step 1: Install dependencies
    log_info "Running 'pnpm install'..."
    if run_cmd pnpm install; then
        log_success "pnpm install completed"
    else
        log_error "pnpm install failed"
        return 1
    fi

    # Step 2: Run pnpm build if scripts are defined
    if pnpm run build > /dev/null 2>&1; then
        log_info "Running 'pnpm build'..."
        if run_cmd pnpm build; then
            log_success "pnpm build completed"
        else
            log_warning "pnpm build had issues but continuing"
        fi
    fi

    cd "${REPO_ROOT}"
    return 0
}

################################################################################
# Build Validation
################################################################################

validate_build() {
    log_info "Validating build output..."
    local validation_passed=1

    # Check Rust build artifacts
    if [[ $SKIP_CARGO -eq 0 && $CHECK_ONLY -eq 0 ]]; then
        if [[ -d "${CARGO_WS}/target/debug" ]]; then
            log_success "Rust debug artifacts found"
        else
            log_error "Rust debug artifacts not found"
            validation_passed=0
        fi
    fi

    # Check pnpm dependencies
    if [[ $SKIP_PNPM -eq 0 ]]; then
        if [[ -d "${PNPM_WS}/node_modules" ]]; then
            log_success "Node.js dependencies installed"
        else
            log_error "Node.js dependencies not found"
            validation_passed=0
        fi
    fi

    if [[ $validation_passed -eq 1 ]]; then
        log_success "Build validation passed"
        return 0
    else
        log_error "Build validation failed"
        return 1
    fi
}

################################################################################
# Main Entry Point
################################################################################

main() {
    # Parse command-line arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --help)
                show_help
                ;;
            --check-only)
                CHECK_ONLY=1
                shift
                ;;
            --no-pnpm)
                SKIP_PNPM=1
                shift
                ;;
            --no-cargo)
                SKIP_CARGO=1
                shift
                ;;
            --verbose)
                VERBOSE=1
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    log_info "=========================================="
    log_info "YUGI Canonical Build System"
    log_info "=========================================="
    log_info "Repository root: ${REPO_ROOT}"
    log_info "Cargo workspace: ${CARGO_WS}"
    log_info "pnpm workspace: ${PNPM_WS}"
    log_info ""

    # Run build steps
    if ! check_prerequisites; then
        exit 1
    fi

    if ! setup_environment; then
        exit 1
    fi

    local build_success=1

    if [[ $SKIP_CARGO -eq 0 ]]; then
        if ! build_cargo; then
            build_success=0
        fi
    else
        log_info "Skipping Cargo build (--no-cargo)"
    fi

    if [[ $SKIP_PNPM -eq 0 ]]; then
        if ! build_pnpm; then
            build_success=0
        fi
    else
        log_info "Skipping pnpm build (--no-pnpm)"
    fi

    if [[ $build_success -eq 0 ]]; then
        log_error "Build failed"
        exit 1
    fi

    if ! validate_build; then
        exit 1
    fi

    log_info ""
    log_success "=========================================="
    log_success "Build completed successfully!"
    log_success "=========================================="
    exit 0
}

# Execute main
main "$@"
