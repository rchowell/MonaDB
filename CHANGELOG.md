# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking:** `Connection` no longer supports `db["table"]`; use `db.table("table")` to obtain a table handle.
- Documentation and examples standardized on the `db` variable name instead of `con` or `conn`.

## [0.1.0] - 2026-03-22

### Added

- Embedded LMDB storage engine with order-preserving key encoding.
- SQL lexer, parser, binder, compiler, and stack-based VM.
- Python package (`monadb`) with DuckDB-style `Connection` API.
- Interactive `monadb` REPL shell with syntax highlighting and multiline input.
- Caret-annotated syntax errors and user-facing runtime error messages.
- Catalog system table for schema metadata.
- Distribution via crates.io, PyPI, GitHub Releases, and Homebrew (`monadb` formula and binary).

[0.1.0]: https://github.com/rchowell/MonaDB/releases/tag/v0.1.0
