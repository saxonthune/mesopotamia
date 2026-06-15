---
name: rust-idioms
description: Rust idiom and API-design reference — naming conventions, common idioms, anti-patterns, and using clippy/rustfmt as the idiomaticness oracle. Use when writing, reviewing, or judging whether Rust code is idiomatic.
---

# Rust Idioms

A condensed reference for writing and judging **idiomatic Rust**. It distills two community sources of truth — the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and [Rust Design Patterns](https://rust-unofficial.github.io/patterns/) — and pairs them with the two tools that *enforce* idiom on real code.

This skill is general-purpose: it describes idiomatic Rust, not any one project.

## The Oracles Come First

Before reasoning about idiom from memory, lean on the tools. They are authoritative, current, and specific to the actual code.

### clippy — the idiomaticness linter

`cargo clippy` is the single best answer to "is this idiomatic?". It carries 700+ lints, each with a rationale and a suggested rewrite.

```sh
cargo clippy                      # lint the default target set
cargo clippy --all-targets        # include tests, examples, benches
cargo clippy --fix                # auto-apply the machine-applicable suggestions
cargo clippy -- -D warnings       # treat every lint as an error (CI gate)
```

Lint groups, from always-on to opt-in:

- `clippy::all` — on by default: **correctness**, **suspicious**, **style**, **complexity**, **perf**.
- `clippy::pedantic` — stricter style/idiom; opt in with `cargo clippy -- -W clippy::pedantic` or `#![warn(clippy::pedantic)]`. Expect false positives; `#[allow(...)]` the ones that don't fit.
- `clippy::nursery` (unstable lints) and `clippy::cargo` (manifest lints) — opt in as desired.

Silence a single lint where you genuinely disagree, with a reason:

```rust
#[allow(clippy::too_many_arguments)] // builder would obscure the call site here
fn f(/* ... */) {}
```

Every lint is documented (with "why" and "instead") at <https://rust-lang.github.io/rust-clippy/master>. When clippy fires, read the lint page rather than guessing.

### rustfmt — the formatting authority

`cargo fmt` is the style oracle for layout. Do not hand-argue brace placement, import ordering, or line wrapping — run the formatter and accept its output. Project overrides live in `rustfmt.toml`.

## Quick Idiom Checklist

The hits that come up most when reviewing for idiom:

- **Naming follows convention** — `snake_case` items, `CamelCase` types, `SCREAMING_SNAKE_CASE` consts; conversions named `as_`/`to_`/`into_` by cost and ownership. → `references/naming.md`
- **Constructors are `fn new() -> Self`**, and types that can default implement `Default` (often `#[derive(Default)]`). → `references/idioms.md`
- **Derive the common traits eagerly** — `Debug` on all public types; `Clone`, `PartialEq`, `Default`, etc. where they make sense. → `references/api-design.md`
- **Errors propagate with `?`**, not `unwrap`/`expect` in library code; functions return `Result`.
- **Prefer iterators and combinators** (`map`/`filter`/`collect`) over manual index loops; `Option`/`Result` have rich combinators.
- **Encode meaning in types** — newtypes and enums over `bool`/`Option`/stringly-typed args. → `references/api-design.md`
- **A `.clone()` added just to silence the borrow checker is a smell.** → `references/anti-patterns.md`
- **`Deref` is for smart pointers, not inheritance.** → `references/anti-patterns.md`

## References

Load by concern:

- **`references/naming.md`** — casing (RFC 430), conversion prefixes, getters, iterator method/type names, word order.
- **`references/idioms.md`** — constructors, `Default`, builder, newtype, `mem::take`/`replace`, `format!`, `Option` as iterator, temporary immutability, RAII guards.
- **`references/anti-patterns.md`** — clone-to-satisfy-the-borrow-checker, `Deref` polymorphism, `#![deny(warnings)]`.
- **`references/api-design.md`** — the API Guidelines checklist, condensed: traits to implement, conversions, type safety, predictability, future-proofing.

## Vendored Source

The full upstream books are cached (gitignored) under `.temp/api-guidelines/` and `.temp/patterns/`. When a reference here is too terse, read the matching chapter there rather than inventing detail.
