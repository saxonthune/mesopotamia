# Common Idioms

Condensed from Rust Design Patterns (idioms + creational/behavioural patterns). These are the constructs idiomatic Rust reaches for.

## Constructors: `new`

Rust has no constructor language feature. The convention is an associated function `new` returning `Self`:

```rust
impl Second {
    pub fn new(value: u64) -> Self {
        Self { value }
    }
    pub fn value(&self) -> u64 { self.value }
}
```

Users expect `new` to exist; provide it whenever a no-/few-argument constructor is reasonable.

## `Default`

`Default` abstracts "the zero value" so a type works with `unwrap_or_default`, `..Default::default()`, and generic containers. Derive it when every field is `Default`:

```rust
#[derive(Default, Debug, PartialEq)]
struct Config {
    output: Option<PathBuf>,   // None
    search_path: Vec<PathBuf>, // empty
    timeout: Duration,         // zero
    check: bool,               // false
}

let conf = Config { check: true, ..Default::default() };
```

It is common and expected to implement **both** `Default` and an empty `new()`. Unlike `new`, there is exactly one `Default` per type and it takes no arguments.

## Builder

For types with many or optional fields (Rust lacks overloading and default args), build incrementally then finalize:

```rust
impl Foo {
    pub fn builder() -> FooBuilder { FooBuilder::default() }
}

#[derive(Default)]
pub struct FooBuilder { bar: String }

impl FooBuilder {
    pub fn name(mut self, bar: String) -> FooBuilder { self.bar = bar; self }
    pub fn build(self) -> Foo { Foo { bar: self.bar } }
}

let foo = Foo::builder().name("Y".into()).build();
```

Taking/returning the builder by `&mut self` is also fine and lets you split the calls. `std::process::Command` is the canonical example. The `derive_builder` crate removes the boilerplate.

## Newtype

A single-field tuple struct makes an opaque, zero-cost wrapper — a *new* type, not a `type` alias:

```rust
struct Password(String);

impl Display for Password {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "****************")
    }
}
```

Uses: enforce type distinctions (`Miles` vs `Kilometres` over `f64`), hide an implementation type behind a public API, restrict the exposed interface, or flip Copy semantics to move. The cost is boilerplate: every wrapped method/trait you want needs a pass-through. The wrapped field is private by default, which is the point.

## `mem::take` / `mem::replace` — change owned values behind `&mut`

To move an owned value out of a `&mut` (e.g. swap an enum variant) without cloning, swap something back in:

```rust
fn a_to_b(e: &mut MyEnum) {
    if let MyEnum::A { name, x: 0 } = e {
        *e = MyEnum::B { name: mem::take(name) }; // leaves an empty String behind
    }
}
```

`mem::take(x)` replaces `x` with its `Default` and returns the old value (empty `String` doesn't allocate). `mem::replace(x, v)` lets you choose the replacement — use it when the type isn't `Default`. For `Option`, prefer its own `.take()`. This is the idiomatic cure for the clone-to-satisfy-the-borrow-checker anti-pattern.

## `format!` for string building

When mixing literals and values, `format!` is the readable choice:

```rust
fn say_hello(name: &str) -> String { format!("Hello {name}!") }
```

`push`/`push_str` on a pre-allocated `String` is more efficient in hot paths, but `format!` wins on clarity by default.

## `Option` as a zero-or-one iterator

`Option` implements `IntoIterator`:

```rust
logicians.extend(turing);                       // pushes the Some value, if any
for x in base.iter().chain(option.iter()) { }   // append an optional tail
```

For an always-`Some` value, `std::iter::once(x)` is clearer. To iterate a single `Option`, prefer `if let Some(..)` over a `for` loop.

## Temporary mutability

When data is mutable only during setup, make the immutability explicit by rebinding or scoping:

```rust
let data = { let mut data = get_vec(); data.sort(); data }; // data now immutable
// or
let mut data = get_vec();
data.sort();
let data = data;                                            // shadow as immutable
```

## RAII guards

Acquire a resource in a constructor, release it in `Drop`, and hand out a *guard* that mediates access. The borrow checker guarantees references obtained through the guard cannot outlive it — `MutexGuard` is the archetype. Implementing `Deref` on the guard is an ergonomic nicety, not the essence; a `get` method works as well.
