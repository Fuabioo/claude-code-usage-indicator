# Cross-platform task runner for the Claude Code usage workspace.
#
# Shared recipes live here; OS-specific recipes are split into linux.just and macos.just
# and gated with [linux] / [macos] attributes. `just build` / `just install` dispatch to
# the right platform automatically via os().

import 'linux.just'
import 'macos.just'

# Show available recipes
default:
    @just --list

# Test the cross-platform crates (core + CLI); applet excluded — see `test-all` on Linux.
test:
    cargo test

# Run tests across the entire workspace (Linux only — pulls in the COSMIC applet).
[linux]
test-all:
    cargo test --workspace

# Build the cross-platform CLI (release).
build-cli:
    cargo build -p cc-usage-cli --release

# Build everything for the current OS (CLI + the platform's GUI).
build: build-cli
    @just build-{{ os() }}

# Install the GUI for the current OS.
install:
    @just install-{{ os() }}

# Remove build artifacts (Rust + Swift).
clean:
    cargo clean
    rm -rf macos/CcUsageMenuBar/.build target/macos
