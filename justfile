# Task runner for mesopotamia. Run `just` or `just --list` to see recipes.

# Default recipe: list everything.
default:
    @just --list

# Run the egui/Bevy demo (dynamic linking for faster iterative builds).
demo1:
    cargo run --features bevy/dynamic_linking --bin demo1

# Pure-function/unit tests only, skipping the slow Bevy-linking integration
# binaries. With a warm target dir it finishes in seconds.

# Fast unit-test suite; the suite headless agents run to self-verify.
test-fast:
    cargo test --lib

# Adds macro_sim's crossing/ecology canaries and balance checks on top of the
# unit tests. Slower.

# Full test suite — units plus integration invariants; the pre-merge gate.
test-all:
    cargo test

# Stage 1 is the syn extractor over the source tree; stage 2 shapes its JSON
# into graph + pack. The extractor is a standalone tool crate, kept out of the
# game's (wasm) build on purpose.

# Regenerate the Luminous structure canvas (override dir: `just canvas src/elk`).
canvas dir="src":
    cargo run --manifest-path tools/luminous-extractor/Cargo.toml -- {{dir}} .canvases/rust-structure.json
    tsx .canvases/rust-structure.pipeline.ts
