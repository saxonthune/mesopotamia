.PHONY: demo1 test-fast test-all

demo1:
	cargo run --features bevy/dynamic_linking --bin demo1

# Fast suite — pure-function/unit tests only (`--lib`), skipping the slow
# Bevy-linking integration binaries (macro_sim, balance). This is the suite a
# headless todo-task agent runs to self-verify: it compiles only our crate's
# unit tests, so with a warm target dir it finishes in seconds.
test-fast:
	cargo test --lib

# Broad suite — unit tests plus the integration behavioral invariants
# (macro_sim's crossing/ecology canaries, balance). Slower; this is the
# pre-merge gate we run before landing an agent's work.
test-all:
	cargo test
