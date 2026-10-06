#!/usr/bin/env bash
# Build the documentation site: the mdBook guide at the root and the
# rustdoc API reference under /api. Output goes to target/site.
set -euo pipefail
SITE=target/site
# Start from an empty directory so removed pages don't linger.
if [ -d "$SITE" ]; then mv "$SITE" "$(mktemp -d)/"; fi
mdbook build -d "$PWD/$SITE"
RUSTDOCFLAGS="${RUSTDOCFLAGS:-} --cfg docsrs" \
  cargo +nightly doc --no-deps --features microlp
mkdir -p "$SITE/api"
cp -r target/doc/. "$SITE/api/"
# rustdoc has no root index; send /api/ to the crate docs.
echo '<meta http-equiv="refresh" content="0; url=quoracle/index.html">' \
  > "$SITE/api/index.html"
touch "$SITE/.nojekyll"
