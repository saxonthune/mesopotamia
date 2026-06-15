# API Design Checklist

The Rust API Guidelines checklist, condensed. Each item carries its `[C-*]` tag — search the full text under `.temp/api-guidelines/src/` (or <https://rust-lang.github.io/api-guidelines/>) for rationale and examples. Naming items live in `naming.md`.

## Interoperability — play well with the ecosystem

- **Eagerly implement common traits** (C-COMMON-TRAITS): `Copy`, `Clone`, `Eq`, `PartialEq`, `Ord`, `PartialOrd`, `Hash`, `Debug`, `Display`, `Default` — wherever each is meaningful. They cannot be added by downstream users, so omitting one is a real limitation.
- **Conversions use the standard traits** (C-CONV-TRAITS): `From`, `TryFrom`, `AsRef`, `AsMut`. Implement `From` (not `Into`) — the `Into` impl comes for free.
- **Collections implement `FromIterator` and `Extend`** (C-COLLECT).
- **Types are `Send`/`Sync` where possible** (C-SEND-SYNC).
- **Error types are meaningful and well-behaved** (C-GOOD-ERR): implement `Error`, `Display`, `Debug`; carry context.
- **Generic reader/writer fns take `R: Read` / `W: Write` by value** (C-RW-VALUE).

## Type Safety — let the compiler carry meaning

- **Newtypes for static distinctions** (C-NEWTYPE): wrap to make incompatible things incompatible.
- **Arguments convey meaning through types, not `bool`/`Option`** (C-CUSTOM-TYPE): a two-state argument is a named two-variant enum, so call sites read clearly (`Direction::Ascending`, not `true`).
- **Sets of flags are `bitflags`, not enums** (C-BITFLAG).
- **Builders for complex construction** (C-BUILDER). See `idioms.md`.

## Predictability — code acts how it looks

- **Constructors are static inherent methods** (C-CTOR): `Type::new(...)`.
- **Functions with a clear receiver are methods** (C-METHOD): `foo.bar()` over `bar(foo)`.
- **No out-parameters** (C-NO-OUT): return a tuple/struct instead of writing through `&mut` params.
- **Only smart pointers implement `Deref`/`DerefMut`** (C-DEREF) — see the `Deref` anti-pattern.
- **Operator overloads are unsurprising** (C-OVERLOAD); **smart pointers don't add inherent methods** (C-SMART-PTR).

## Dependability — unlikely to do the wrong thing

- **Validate arguments** (C-VALIDATE).
- **Destructors never fail** (C-DTOR-FAIL); destructors that may block offer an explicit alternative (C-DTOR-BLOCK).

## Debuggability

- **All public types implement `Debug`** (C-DEBUG), and the representation is **never empty** (C-DEBUG-NONEMPTY).

## Future-Proofing — improve without breaking users

- **Structs have private fields** (C-STRUCT-PRIVATE): expose accessors, keep layout free to change.
- **Sealed traits** where downstream impls should be impossible (C-SEALED).
- **Newtypes encapsulate implementation details** (C-NEWTYPE-HIDE).
- **Don't duplicate derived trait bounds** on the struct definition (C-STRUCT-BOUNDS) — bound the impls, not the type.

## Documentation

- **Thorough crate-level docs with examples** (C-CRATE-DOC); **every public item has a rustdoc example** (C-EXAMPLE).
- **Examples use `?`**, not `unwrap`/`try!` (C-QUESTION-MARK).
- **Document errors, panics, and safety** (C-FAILURE): `# Errors`, `# Panics`, `# Safety` sections.
- **`Cargo.toml` carries full metadata** (C-METADATA): description, license, repository, keywords, categories.

## Necessities

- **Permissive license** (C-PERMISSIVE), typically dual MIT/Apache-2.0.
- **Public deps of a stable crate are themselves stable** (C-STABLE).
