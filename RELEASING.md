# Releasing

MonaDB ships one artifact: the `monadb` package on PyPI. The version lives in
`Cargo.toml`; maturin reads it through `dynamic = ["version"]` in
`pyproject.toml`.

## Steps

1. Bump `version` in `Cargo.toml`.
2. Add the release section to `CHANGELOG.md`.
3. Verify locally:

   ```sh
   cargo test
   cargo clippy --all-targets -- -D warnings
   maturin develop && python -m pytest tests -q
   ```

4. Commit, then tag and push:

   ```sh
   git tag -a vX.Y.Z -m "MonaDB X.Y.Z"
   git push origin main --tags
   ```

The `v*` tag triggers `.github/workflows/release.yml`, which runs the suite,
builds wheels for Linux (x86_64, aarch64), macOS (arm64, x86_64), and Windows
(x64) plus an sdist, and publishes to PyPI via trusted publishing from the
`pypi` environment.

## Checking the build

```sh
maturin build --release
pip install target/wheels/*.whl
```

Wheels are `abi3-py39`: one wheel per platform covers every supported Python.
