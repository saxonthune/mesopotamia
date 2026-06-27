# Task runner for mesopotamia. Run `just` or `just --list` to see recipes.

# Default recipe: list everything.
default:
    @just --list

# Run the egui/Bevy demo (dynamic linking for faster iterative builds).
demo1:
    cargo run --features bevy/dynamic_linking --bin demo1

# Run the game on a hand-built test map instead of the procedural world. Map
# names come from `worldgen::testmap::TEST_MAPS`.
# Usage: just testmap            (defaults to oval-gaps)
#        just testmap oval-gaps
testmap name="oval-gaps":
    cargo run --features bevy/dynamic_linking --bin demo1 -- --test-map {{name}}

# Run the Driftscape terminal screensaver (Starliner). It's a standalone crate
# off the game's build, so it links no Bevy. Quit with q/Esc/Ctrl-C.
demo3:
    cargo run --manifest-path driftscape/Cargo.toml --bin demo3

# Pure-function/unit tests only, skipping the slow Bevy-linking integration
# binaries. With a warm target dir it finishes in seconds.

# Fast unit-test suite; the suite headless agents run to self-verify. Covers the
# game's pure-fn lib and the standalone driftscape crate (both link no Bevy).
test-fast:
    cargo test --lib
    cargo test --manifest-path driftscape/Cargo.toml

# Adds macro_sim's crossing/ecology canaries and balance checks on top of the
# unit tests. Slower.

# Full test suite — units plus integration invariants; the pre-merge gate.
test-all:
    cargo test
    cargo test --manifest-path driftscape/Cargo.toml

# Run an #[ignore] investigation probe in RELEASE — the headless sim is compute-
# bound (whole-grid growth + per-elk grass gradient each tick) and already runs at
# max tick rate, so the only real speedup is optimised codegen (~10× over debug).
# Usage: just probe working_presets_probe   (any probe fn name in tests/herd_shape.rs)
probe name:
    cargo test --release --test herd_shape {{name}} -- --ignored --nocapture

# Run the headless scenario harness and print metric projections for an agent to read:
# the per-metric summary always, the full CSV dump with `csv=1`.
# Usage: just probe-scenario          (summary table)
#        just probe-scenario csv=1    (full CSV time series)
probe-scenario csv="0":
    cargo test --release --test scenarios {{ if csv == "1" { "oval_gaps_dump_csv -- --ignored" } else { "oval_gaps_records_a_spawned_wave --" } }} --nocapture

# Wraps build-web.sh. Prereqs: `cargo install wasm-bindgen-cli --version 0.2.125`
# (must match the wasm-bindgen dep) and, optionally, `cargo install wasm-opt`.

# Build the wasm demos and assemble the static site into dist/ (gitignored).
web:
    ./build-web.sh

# Fast debug wasm build for the tweak/refresh loop — skips release opt + wasm-opt.
# Bigger wasm, much shorter compile. Use `just web` for the deploy artifact.
web-dev:
    ./build-web.sh dev

# Browsers block wasm/ES-module loads over file://, so a real server is required.

# Serve the assembled dist/ over http (don't open the html directly).
web-serve:
    @echo "Serving dist/ on http://localhost:8000  (Ctrl-C to stop)"
    cd dist && python3 -m http.server 8000

# Stage 1 is the syn extractor over the source tree; stage 2 shapes its JSON
# into graph + pack. The extractor is a standalone tool crate, kept out of the
# game's (wasm) build on purpose.

# Regenerate the Luminous structure canvas (override dir: `just canvas src/elk`).
canvas dir="src":
    cargo run --manifest-path tools/luminous-extractor/Cargo.toml -- {{dir}} .canvases/rust-structure.json
    tsx .canvases/rust-structure.pipeline.ts
