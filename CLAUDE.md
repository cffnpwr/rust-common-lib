# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Building and Testing
- `cargo build` - Build all workspace members
- `cargo test` - Run all tests across workspace
- `cargo test -p <package-name>` - Run tests for specific package (patricia-tree, auto-impl-macro)
- `cargo check` - Quick check without full build
- `cargo fmt` - Format code (follows rustfmt.toml config)
- `cargo clippy` - Run linter

### Coverage
- `cargo llvm-cov` - Generate test coverage report (uses mise-installed cargo-llvm-cov)

### Individual Package Development
- `cargo test -p patricia-tree` - Test Patricia tree implementation
- `cargo test -p auto-impl-macro` - Test procedural macros
- `cargo build -p <package>` - Build specific package

## Architecture

### Workspace Structure
This is a Rust workspace containing multiple library crates:

- **Root crate (`common-lib`)**: Feature-gated re-exports of workspace members
- **`patricia-tree`**: Patricia Trie data structure implementation with insertion and search
- **`auto-impl-macro`**: Procedural macro for automatic trait implementations

### Library Distribution Model
Libraries are distributed via Git dependency with optional features:
```toml
# Use as workspace with features
common-lib = { git = "...", features = ["patricia-tree"] }

# Or use individual libraries directly
patricia-tree = { git = "..." }
```

### Key Implementation Details
- **Patricia Tree**: Implements prefix-based tree with HashMap children, handles prefix splitting for insertions
- **Auto Impl Macro**: Derive macro `AutoTryFrom` with `#[auto_try_from]` attributes for generating TryFrom implementations
- **Code Style**: Uses grouped imports (`StdExternalCrate`) and module-level granularity per rustfmt.toml

### Environment
- Uses mise for tool management (nightly Rust, cargo-llvm-cov)
- Rust edition 2024 with minimum version 1.85.0
- Workspace dependencies managed centrally (anyhow, thiserror)