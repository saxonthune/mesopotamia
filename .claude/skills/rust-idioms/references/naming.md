# Naming Conventions

Condensed from the Rust API Guidelines "Naming" chapter (RFC 430). clippy enforces much of this (`clippy::all` style group) — run it.

## Casing (C-CASE)

Type-level constructs use `UpperCamelCase`; value-level constructs use `snake_case`.

| Item | Convention |
|---|---|
| Modules | `snake_case` |
| Types, Traits, Enum variants | `UpperCamelCase` |
| Functions, Methods, Local variables | `snake_case` |
| Statics, Constants | `SCREAMING_SNAKE_CASE` |
| Type parameters | concise `UpperCamelCase`, usually a single letter: `T` |
| Lifetimes | short lowercase: `'a`, `'de`, `'src` |
| Macros | `snake_case!` |
| General constructor | `new` or `with_more_details` |
| Conversion constructor | `from_some_other_type` |

In `UpperCamelCase`, treat an acronym as one word: `Uuid` not `UUID`, `Stdin` not `StdIn`. In `snake_case`, acronyms lowercase fully: `is_xid_start`. A "word" is never a single letter except as the last word: `btree_map` not `b_tree_map`, but `PI_2` not `PI2`.

Crate names do not carry `-rs`/`-rust` suffixes — every crate is Rust.

## Conversion Prefixes (C-CONV)

Name conversion methods by cost and ownership:

| Prefix | Cost | Ownership |
|---|---|---|
| `as_` | Free | borrowed → borrowed |
| `to_` | Expensive | borrowed → borrowed, borrowed → owned (non-Copy), owned → owned (Copy) |
| `into_` | Variable | owned → owned (non-Copy), consumes `self` |

Examples: `str::as_bytes()` (free view), `Path::to_str()` (runtime UTF-8 check, so `to_` not `as_`), `str::to_lowercase()` (allocates), `String::into_bytes()` (consumes). A single-value wrapper exposes its inner value via `into_inner()`.

When `mut` is part of the return type, place it as it appears in the type: `as_mut_slice` (returns `&mut [T]`), not `as_slice_mut`.

## Getters (C-GETTER)

No `get_` prefix. The field accessor is named for the field:

```rust
pub fn first(&self) -> &First { &self.first }
pub fn first_mut(&mut self) -> &mut First { &mut self.first }
```

`get` is reserved for the one obvious thing to fetch (e.g. `Cell::get`). Indexed access pairs with unsafe `_unchecked` variants: `get`/`get_mut` return `Option`, `get_unchecked`/`get_unchecked_mut` are `unsafe`.

## Iterator Methods and Types (C-ITER, C-ITER-TY)

A homogeneous collection of `U` provides:

```rust
fn iter(&self) -> Iter            // Iterator<Item = &U>
fn iter_mut(&mut self) -> IterMut // Iterator<Item = &mut U>
fn into_iter(self) -> IntoIter    // Iterator<Item = U>
```

The returned type name matches the method: `into_iter()` → `IntoIter`, `keys()` → `Keys`. These read best module-qualified, e.g. `vec::IntoIter`.

## Cargo Features (C-FEATURE)

Feature names carry meaning — `abc`, not `use-abc` or `with-abc`. Features must be additive, so a negative name like `no-abc` is almost never correct. The optional-std feature is conventionally named `std`.

## Word Order (C-WORD-ORDER)

Pick a word order and hold it across the crate. The standard library uses verb-object-error: `ParseIntError`, `JoinPathsError` — so a new parse error is `ParseAddrError`, not `AddrParseError`. The specific order matters less than consistency with the crate and with std.
