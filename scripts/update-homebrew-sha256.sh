#!/usr/bin/env bash
# Print sha256 sums for Homebrew formula after a GitHub Release.
# Usage: VERSION=0.1.0 ./scripts/update-homebrew-sha256.sh

set -euo pipefail

VERSION="${VERSION:-0.1.0}"
BASE="https://github.com/rchowell/MonaDB/releases/download/v${VERSION}"

for target in \
  aarch64-apple-darwin \
  x86_64-apple-darwin \
  aarch64-unknown-linux-gnu \
  x86_64-unknown-linux-gnu
do
  url="${BASE}/mona-${target}.tar.gz"
  echo "# ${target}"
  echo "url \"${url}\""
  if command -v curl >/dev/null 2>&1; then
  curl -fsSL "${url}" | shasum -a 256
  else
    echo "# (install curl to fetch sha256)"
  fi
  echo
done

echo "Update homebrew-tap/Formula/monadb.rb with the values above."
