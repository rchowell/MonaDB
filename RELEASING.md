# Releasing MonaDB

This project publishes **four artifacts** from one git tag:

| Channel | Artifact | Trigger |
|---------|----------|---------|
| [crates.io](https://crates.io/crates/monadb) | Rust library | `cargo publish` in CI |
| [PyPI](https://pypi.org/project/monadb/) | Python wheel | `maturin upload` in CI |
| [GitHub Releases](https://github.com/rchowell/MonaDB/releases) | `monadb` CLI tarballs | CI upload |
| [Homebrew tap](https://github.com/rchowell/homebrew-tap) | `monadb` formula | Manual bump after release |

Version is defined in [`Cargo.toml`](Cargo.toml); PyPI reads it via `dynamic = ["version"]` in [`pyproject.toml`](pyproject.toml).

---

## One-time registry setup

### crates.io

1. Create an account at <https://crates.io/>.
2. Verify the crate name `monadb` is available: `cargo search monadb`.
3. Run locally once: `cargo login` (paste API token).
4. Dry-run: `cargo publish --dry-run --no-default-features`.
5. Add GitHub repository secret **`CARGO_REGISTRY_TOKEN`** (crates.io API token).

### PyPI

**Option A — Trusted publishing (recommended)**

1. Create the `monadb` project on PyPI (first upload can create it via CI).
2. On PyPI → Your project → Publishing → Add GitHub Actions publisher:
   - Owner: `rchowell`
   - Repository: `MonaDB`
   - Workflow: `release.yml`
   - Environment: `pypi` (optional)

**Option B — API token**

1. Create a PyPI API token scoped to `monadb`.
2. Add GitHub secret **`PYPI_API_TOKEN`**.

### GitHub

Repository secrets (Settings → Secrets → Actions):

| Secret | Purpose |
|--------|---------|
| `CARGO_REGISTRY_TOKEN` | `cargo publish` |
| `PYPI_API_TOKEN` | Only if not using trusted publishing |

`GITHUB_TOKEN` is provided automatically for release uploads.

### Homebrew tap

1. Create public repo `github.com/rchowell/homebrew-tap`.
2. Copy [`homebrew-tap/Formula/monadb.rb`](homebrew-tap/Formula/monadb.rb) into that repo.
3. Users install with:

   ```sh
   brew tap rchowell/tap
   brew install monadb
   monadb --version
   ```

---

## Release checklist

### Before tagging

1. Update `version` in [`Cargo.toml`](Cargo.toml).
2. Add a dated section to [`CHANGELOG.md`](CHANGELOG.md).
3. Open PR; confirm [`.github/workflows/ci.yml`](.github/workflows/ci.yml) is green on `main`.
4. Local smoke checks:

   ```sh
   cargo test
   cargo build --release --features cli --bin monadb
   ./target/release/monadb --version
   maturin build --release --features python
   cargo publish --dry-run --no-default-features
   ```

### Tag and publish

```sh
git tag v0.1.0
git push origin v0.1.0
```

The [`release.yml`](.github/workflows/release.yml) workflow will:

1. Run tests.
2. Build `monadb` for macOS (arm64, x86_64) and Linux (arm64, x86_64).
3. Build and upload PyPI wheels.
4. `cargo publish --no-default-features` to crates.io.
5. Create a GitHub Release with CLI tarballs.

Monitor the Actions tab until all jobs succeed.

### After GitHub Release

1. **Verify crates.io**: `cargo install monadb --version X.Y.Z --features cli`
2. **Verify PyPI**: `pip install monadb==X.Y.Z` then `python -c "import monadb"`
3. **Verify CLI assets**: download a release tarball and run `./monadb --version`
4. **Update Homebrew formula** in `rchowell/homebrew-tap`:

   ```sh
   cd homebrew-tap
   # For each platform tarball:
   curl -sL <release-url> | shasum -a 256
   # Edit Formula/monadb.rb: version, sha256 values
   git commit -am "monadb X.Y.Z"
   git push
   ```

   Or run [`scripts/update-homebrew-sha256.sh`](scripts/update-homebrew-sha256.sh) after setting `VERSION`.

---

## GitHub Release notes template

```markdown
## Install

**Python:** `pip install monadb==X.Y.Z`

**Rust:** `cargo add monadb@X.Y.Z` or `cargo install monadb --version X.Y.Z --features cli`

**Homebrew:** `brew tap rchowell/tap && brew upgrade monadb`

**CLI binaries** (this page) — extract and run `monadb`.

## Highlights

- (copy from CHANGELOG)
```

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| `cargo publish` fails: crate already exists | Bump version; you cannot republish the same version |
| maturin upload 403 | Check PyPI trusted publishing or `PYPI_API_TOKEN` |
| Conformance tests fail in CI | Six known failures; CI runs them with `continue-on-error` until fixed |
| Homebrew `sha256 mismatch` | Re-download tarball URL from the exact release tag |
