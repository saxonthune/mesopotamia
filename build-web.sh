#!/usr/bin/env bash
# Build the wasm demos and assemble the static site into dist/.
#
# Output layout (everything Cloudflare Pages needs, nothing else):
#   dist/index.html                 — gallery linking to each demo
#   dist/demos/<name>/index.html    — the demo page (canvas + description)
#   dist/demos/<name>/<name>.js     — wasm-bindgen JS glue
#   dist/demos/<name>/<name>_bg.wasm— the compiled app
#
# Deploy with:  npx wrangler pages deploy dist
#
# Prereqs (one-time):
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli   # version MUST match the wasm-bindgen dep
#   cargo install wasm-opt           # (binaryen) optional but strongly advised
set -euo pipefail

# Build mode: `release` (default, optimized + wasm-opt, the deploy artifact) or
# `dev` (debug, no wasm-opt — compiles far faster for the tweak/refresh loop).
MODE="${1:-release}"

# --- the demo list. One entry per binary in src/bin/. Add demos here. ---
DEMOS=(demo1)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST="$ROOT/dist"
if [[ "$MODE" == "dev" ]]; then
    CARGO_PROFILE_FLAGS=()
    TARGET_DIR="$ROOT/target/wasm32-unknown-unknown/debug"
else
    CARGO_PROFILE_FLAGS=(--release)
    TARGET_DIR="$ROOT/target/wasm32-unknown-unknown/release"
fi

echo "==> cleaning dist/"
rm -rf "$DIST"
mkdir -p "$DIST/demos"

# Gallery page sits at the site root.
cp "$ROOT/web/index.html" "$DIST/index.html"

for demo in "${DEMOS[@]}"; do
    echo "==> building $demo ($MODE, wasm32)"
    cargo build "${CARGO_PROFILE_FLAGS[@]}" --bin "$demo" --target wasm32-unknown-unknown

    out="$DIST/demos/$demo"
    mkdir -p "$out"

    echo "==> wasm-bindgen $demo"
    wasm-bindgen --target web --no-typescript \
        --out-dir "$out" --out-name "$demo" \
        "$TARGET_DIR/$demo.wasm"

    if [[ "$MODE" == "dev" ]]; then
        echo "    (dev build — skipping wasm-opt)"
    elif command -v wasm-opt >/dev/null 2>&1; then
        echo "==> wasm-opt -Oz $demo"
        # Rust's wasm32 output uses post-MVP features (bulk memory, sign
        # extension, etc.). wasm-opt must be told to accept them or it rejects
        # the module during validation. These are all baseline in every browser.
        wasm-opt -Oz \
            --enable-bulk-memory \
            --enable-sign-ext \
            --enable-mutable-globals \
            --enable-nontrapping-float-to-int \
            --enable-reference-types \
            -o "$out/${demo}_bg.wasm" "$out/${demo}_bg.wasm"
    else
        echo "    (wasm-opt not found — skipping size optimization)"
    fi

    # Copy the demo's page (canvas + description).
    cp "$ROOT/web/demos/$demo/index.html" "$out/index.html"

    size=$(du -h "$out/${demo}_bg.wasm" | cut -f1)
    echo "==> $demo done — wasm is $size"
done

echo "==> site assembled in dist/. Preview: (cd dist && python3 -m http.server)"
