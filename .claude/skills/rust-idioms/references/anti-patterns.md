# Anti-Patterns

Condensed from Rust Design Patterns "Anti-patterns". These read as plausible but work against the grain of the language. clippy catches some of them directly.

## Clone to satisfy the borrow checker

Reaching for `.clone()` to make a borrow error disappear copies data and silently desynchronizes the two values — they are now independent.

```rust
let mut x = 5;
let y = &mut (x.clone()); // x was never actually borrowed; y mutates a copy
```

A clone added *specifically* to silence the borrow checker is the tell. Clones should be deliberate, with the cost understood. Exceptions: `Rc`/`Arc` clone cheaply (they bump a refcount over shared data), and it is genuinely fine to write an inefficient clone while learning, in prototypes, or when satisfying the checker correctly would hurt readability more than the copy hurts performance.

The targeted fix for moving owned values out of a `&mut` is `mem::take` / `mem::replace` (see `idioms.md`). And `cargo clippy` flags many unnecessary clones for you.

## `Deref` polymorphism — faking inheritance

Implementing `Deref` so a wrapper "inherits" the inner type's methods abuses the trait:

```rust
struct Bar { f: Foo }
impl Deref for Bar {            // anti-pattern: Bar is not a pointer to Foo
    type Target = Foo;
    fn deref(&self) -> &Foo { &self.f }
}
```

It saves a little delegation boilerplate but is surprising and leaky: it is *not* subtyping (traits on `Foo` are not implemented for `Bar`, so it breaks generic/bounded code), `self` inside the method refers to `Foo` not `Bar`, and there is no privacy or multiple-inheritance story. `Deref` is for smart pointers — converting a pointer-to-`T` into a `T`, not converting between unrelated types. Rust favors explicit conversions; implicit deref is deliberately limited to indirection.

Instead: use composition with explicit facade methods, or model the shared behavior as a trait. Delegation crates (`delegate`, `ambassador`) remove the boilerplate without the abuse.

## `#![deny(warnings)]`

Denying *all* warnings crate-wide opts out of Rust's stability promise: lints graduate from `warn` to `deny` over a grace period, and APIs get deprecated — so a future compiler can break your build with no code change. It also disables external lint crates unless callers pass `--cap-lints`.

Instead, decouple the policy from the source:

```sh
RUSTFLAGS="-D warnings" cargo build   # deny in CI / locally, no code change
```

Or `deny` a named, curated set of lints in code rather than the blanket `warnings` group — and leave `deprecated` out of it, since deprecations are expected to grow.
