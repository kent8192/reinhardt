# Macro Usage Guidelines

## Purpose

This file defines the policy for using Reinhardt's macros (notably `#[model(...)]` and `form!`) consistently across the codebase.

---

## `#[model(...)]` Macro

### MU-1 (SHOULD): Do Not Combine with `#[derive(Model)]`

The `#[model(...)]` attribute macro automatically applies `#[derive(Model)]` internally. If `#[derive(Model)]` is *also* written on the same struct, the attribute macro detects this and returns the input unchanged (the existing derive then handles the macro logic). Compilation succeeds today, but the explicit `Model` derive becomes redundant noise that obscures intent.

**Rule:**
- When using `#[model(...)]`, **prefer not** to also write `#[derive(Model)]` on the same struct — the attribute applies it for you.
- Add other derives that `#[model(...)]` does not provide via a separate `#[derive(...)]` (e.g., `Debug`, `Clone`, `serde::Serialize`).
- Existing code that combines both is supported and does not need an immediate fix; new code should follow the canonical form.

**Examples:**

```rust
// ✅ Canonical — let #[model(...)] add the Model derive
#[model(table = "people")]
#[derive(Debug, Clone)]
pub struct Person {
    pub id: i64,
    pub name: String,
}

// ⚠️ Redundant but supported — Model derive duplicates what #[model(...)] applies
#[model(table = "people")]
#[derive(Debug, Clone, Model)]
pub struct Person {
    pub id: i64,
    pub name: String,
}
```

### MU-2 (MUST): Initialize via the Macro-Generated Builder

The `#[model(...)]` macro generates a typestate builder. `Model::build()` is the canonical initialization path; `Model::new()` is a zero-argument alias for `Model::build()`.

**Rule:**
- Initialize `#[model(...)]` structs using the macro-generated `build()` builder or zero-argument `new()` alias.
- Do not call a positional constructor such as `Person::new(id, name)`. The generated `new()` accepts no arguments.
- Do not initialize them via struct literals (`Person { id: 0, name: ... }`) when the macro-generated builder is available.
- Exception: struct-literal syntax may be used in tests when a test specifically needs to set fields that the constructor auto-fills (e.g., to inject a fixed primary key). Document the reason with a comment.

**Examples:**

```rust
// ✅ Correct — use the macro-generated builder
let person = Person::build()
    .name("Alice")
    .finish();

// ✅ Also correct — new() is a zero-argument alias of build()
let person = Person::new()
    .name("Alice")
    .finish();

// ✅ Correct — opt in to overriding a macro-managed field when importing or
// bridging an externally supplied identity.
let person = Person::build()
    .id(existing_id)
    .name("Alice")
    .finish();

// ❌ Incorrect — bypasses macro-generated initialization
let person = Person {
    id: 1,
    name: "Alice".to_string(),
};
```

**Rationale:**
- The generated builder accepts only the fields the user must supply and auto-fills macro-managed fields (auto-generated primary keys, foreign-key id columns, timestamps, relationship markers). Struct-literal initialization forces the caller to spell out every field, including those the builder would have filled — which is brittle.
- Macro-managed database fields also get optional builder setters. Call those setters only when the caller intentionally needs to preserve an externally supplied value, such as an imported primary key or fixed timestamp; omitting the setter keeps the normal generated/default path.
- Centralizing initialization through the builder keeps call sites stable as the macro evolves: adding a required field surfaces as a named setter instead of changing positional argument order.
- Today the generated builder does not perform validation; this rule is about future-proofing and field coverage, not about a current invariant guarantee.

### MU-3 (SHOULD): Prefer `Model::build()` over `Model::new()`

`Model::new()` remains available as a zero-argument alias for `Model::build()` (issue #4401). Both entry points return the same typestate builder. Prefer `build()` in examples and application code because it names the construction pattern directly.

**When to prefer `build()`:**

- Tutorials, examples, and long-lived application code where the model schema
  is expected to evolve. Adding a new required field surfaces as a new setter
  rather than a new positional parameter that breaks every caller in
  lock-step.
- Code that benefits from passing related models by reference: FK setters
  accept any `IntoPrimaryKey<Related>` value (see #4398), so
  `.author(&user)` is exactly as valid as `.author(user_id)`.

**When `new()` is still appropriate:**

- One-shot test fixtures and tight, internal call sites where the shorter alias
  is clearer.
- Migration windows where existing zero-argument construction examples already
  read naturally.

**Examples:**

```rust
// ✅ Typestate builder — each required field named, ordering free, and
// adding a new required field to `Question` keeps this call site compiling.
let question = Question::build()
    .question_text("What's your favorite color?")
    .finish();

// ✅ Zero-argument alias — returns the same builder.
let question = Question::new()
    .question_text("What's your favorite color?")
    .finish();

// ✅ FK setter accepts `&User` directly (composes with #4398).
let choice = Choice::build()
    .choice_text("Red")
    .votes(0)
    .question(&question)
    .finish();
```

**Type-state guarantees:**

- Each required-field setter transitions exactly one slot from `Unset` to
  `Set`. Setters can be called in any order.
- `finish()` is only available when every required slot is `Set`. Calling
  `finish()` with any remaining required setter unused is a **compile-time
  error**, not a runtime panic.
- Macro-managed fields (`auto_now_add`, FK relation fields, identity /
  auto-increment primary keys) are filled in by `finish()` using the
  macro-managed default expressions — no setter call is required.

**Rationale:**

- DESIGN_PHILOSOPHY #5 ("API ergonomics is paramount"): named setters scale
  with model size in a way positional arguments do not.
- DESIGN_PHILOSOPHY #9 ("Every framework eventually becomes outdated"):
  model schemas evolve; `build()` absorbs that evolution without breaking
  callers.
- DESIGN_PHILOSOPHY #4 ("Fail early"): the per-field type-state lifts
  "required field missing" from a runtime error into a compile error.

---

## Quick Reference

### MU-4 (SHOULD): Use Info Companion Type for Cross-Layer Data Transfer

The `#[model]` macro automatically generates a `{Model}Info` companion struct — a plain data carrier with model data fields, lightweight relationship fields, `pub` visibility, and bidirectional `From` conversions.

**Generated for every model by default.** Opt out of only the companion struct
with `#[model(info = false)]`. Use `#[model(server_only)]` for models that
must not expose shared `InfoModel` / `{Model}Info` output on WASM.

```rust
#[model(app_label = "blog", table_name = "posts")]
struct Post {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(max_length = 255)]
    title: String,
    #[field(skip_info = true)]
    internal_cache: String,
    #[rel(foreign_key, related_name = "posts")]
    author: ForeignKeyField<Author>,
}

// Auto-generated:
// PostInfo {
//     id,
//     title,
//     author: RelationInfo<Author>,
// }
// From<Post> for PostInfo ✓
// From<PostInfo> for Post ✓
```

**Field inclusion rules:**
- Regular data fields: included
- `ForeignKeyField<T>` and `OneToOneField<T>`: included as `RelationInfo<T>`
- `ManyToManyField<Source, Target>`: included as `ManyToManyInfo<Source, Target>`
- FK `_id` fields (auto-generated): not exposed directly; use `info.author.id`
- Relationship marker types are not exposed directly because they do not carry values
- `#[field(skip = true)]` or `#[field(skip_info = true)]` fields: excluded

**Builder with relationship payload support:**
```rust
let info = PostInfo::build()
    .id(Some(1))
    .title("Hello")
    .author(&author)  // accepts &Author via IntoPrimaryKey
    .finish();

let info = PostInfo::build()
    .id(Some(1))
    .title("Hello")
    .author(author_uuid)  // also accepts raw PK value
    .finish();
```

Many-to-many Info fields use a lightweight target-primary-key list:

```rust
let info = PostInfo::build()
    .id(Some(1))
    .title("Hello")
    .author(author_uuid)
    .tags([tag_id_1, tag_id_2])
    .finish();

assert_eq!(info.author.id, author_uuid);
assert_eq!(info.tags.target_ids, vec![tag_id_1, tag_id_2]);
```

When serde derives are mirrored onto `{Model}Info`, the relationship payloads
serialize with the same lightweight field names:

```json
{
  "author": { "id": "..." },
  "tags": { "target_ids": ["..."] }
}
```

**Validation auto-generation:**
Validation attributes are derived from `#[field(...)]` config and emitted as `#[cfg_attr(native, validate(...))]`:

| `#[field(...)]` | Generated validation |
|---|---|
| `max_length = N` | `validate(length(max = N))` |
| `min_length = M` | `validate(length(min = M))` |
| `email = true` | `validate(email)` |
| `url = true` | `validate(url)` |
| `min_value = M` / `max_value = N` | `validate(range(min = M, max = N))` |

---

## `form!` Macro

### MU-5 (MUST): Keep Stable Native Widget Coverage Explicit

The `form!` DSL exposes the following native HTML coverage as stable API:

| DSL item | HTML output | Value state |
|---|---|---|
| `MonthInput` | `<input type="month">` | string field |
| `WeekInput` | `<input type="week">` | string field |
| `ResetButton` | `<button type="reset">` | none |
| `Button` | `<button type="button">` | none |
| `ImageInput` | `<input type="image">` | none |
| `Datalist` | `<datalist>` | option source only |
| `OptGroup` | `<optgroup>` | choice grouping only |
| `Output` | `<output>` | none |
| `Meter` | `<meter>` | none |
| `Progress` | `<progress>` | none |

`Datalist` is an option source for compatible inputs, not a value-holding
field. `OptGroup` groups choices inside choice controls and does not introduce
a separate value slot.

`MonthInput` and `WeekInput` are accepted only for raw string fields:
`CharField`, `TextField`, `EmailField`, `UrlField`, `SlugField`, and
`PasswordField`.

### MU-6 (MUST): Validate Typed Native Attributes Against Compatible Controls

Typed native attributes are valid only on controls that support the corresponding
HTML behavior:

| Attribute | Compatible controls |
|---|---|
| `min` / `max` / `step` | number, range, date, time, datetime-local, month, week |
| `size` | text-like inputs |
| `accept` / `capture` | file-like inputs |
| `list` | datalist-compatible text-like inputs |

`multiple` is not accepted as a typed field property yet. Use the
`SelectMultiple` widget for multi-select fields. File-like multi-select remains
deferred until the generated value contract can represent `Vec<File>` instead
of a single `Option<File>`.

### MU-7 (MUST): Treat `CustomWidget` as Experimental

`CustomWidget` is an experimental extension point. Call sites must opt in with
the `experimental` marker and provide an adapter:

```rust,ignore
date_range: CharField {
    widget: CustomWidget(crate::widgets::DateRangePicker) {
        experimental,
        adapter: crate::widgets::DateRangeAdapter,
    },
}
```

The adapter API may change in a minor release with a documented migration path.

### MU-8 (MUST): Render `FieldGroup` as a Semantic Fieldset

`FieldGroup` renders as semantic `<fieldset>` output. When `label` is present,
the label is rendered as a `<legend>` inside the fieldset.

---

### ✅ MUST DO
- Initialize `#[model(...)]` structs via the macro-generated `build()` builder or zero-argument `new()` alias
- Add unrelated derives (e.g., `Debug`, `Clone`) via a separate `#[derive(...)]`
- Keep stable `form!` widget coverage aligned with the documented native HTML output and value state
- Validate typed native form attributes against the compatible control families
- Treat `CustomWidget` as experimental and require the explicit adapter syntax
- Render `FieldGroup` as semantic `<fieldset>` output with `label` mapped to `<legend>`

### ✅ SHOULD DO
- Use `#[model(...)]` alone (do not also write `#[derive(Model)]`) — the attribute applies the derive for you
- Prefer `Model::build()` over the zero-argument `Model::new()` alias in tutorials, examples, and call sites where the model schema is expected to evolve (MU-3)
- Pass FK values via `.<related>(&model)` in `build()` setters when the related instance is already in scope (composes with #4398)
- Use `{Model}Info` for API DTOs and cross-layer data transfer instead of hand-writing parallel structs (MU-4)
- Use `#[field(skip_info = true)]` to exclude sensitive fields (e.g., password hashes) from the Info struct
- Use `#[model(info = false)]` only when the Info struct would be genuinely unused, but the model may still be referenced by shared relationship metadata
- Use `#[model(server_only)]` only for models that are intentionally native-only and should not participate in WASM/shared type contracts

### ❌ NEVER DO
- Initialize `#[model(...)]` structs via struct-literal syntax in production code (use `build()` or zero-argument `new()`)

---

## Related Documentation

- **Module System**: instructions/MODULE_SYSTEM.md
- **Anti-Patterns**: instructions/ANTI_PATTERNS.md
- **Testing Standards**: instructions/TESTING_STANDARDS.md
