# Native component testing

The `testing` feature provides an in-memory component harness that executes the
same typed standard-event handlers used by browser mounts. It is intended for
component behavior, propagation, reactive rerenders, and deterministic async
settling without Chrome.

```rust,ignore
use reinhardt_pages::event::{ClickEvent, InputEvent};
use reinhardt_pages::prelude::*;
use reinhardt_pages::testing::component::{EventFixture, Role, render};

let name = Signal::new(String::new());
let screen = render(page!({
    input {
        aria_label: "Name",
        @input: {
            let name = name.clone();
            move |event: InputEvent| {
                name.set(event.value().expect("input value"));
            }
        },
    }
    button { @click: |event: ClickEvent| { event.prevent_default(); }, "Save" }
}));

screen
    .get_by_label("Name")
    .dispatch(EventFixture::input().value("Ada"))?;
screen.get_by_role(Role::Button, "Save").click();
screen.settle();
assert_eq!(name.get(), "Ada");
# Ok::<(), reinhardt_pages::testing::component::EventError>(())
```

## Fixtures and targets

`EventFixture::new(KnownEvent)` derives its payload family, bubbling,
cancelability, composition, and deterministic mouse defaults from the event
catalog. Convenience constructors cover `click`, `submit`, `input`, `change`,
`key_down`, and `pointer_move`. `EventFixture::custom(name)` dispatches a plain
named event unless a custom detail setter is used.

Use `.custom_detail(&detail)` to serialize a typed browser `CustomEvent.detail`
payload for a typed `@custom::<Detail>("name")` handler:

```rust,ignore
use reinhardt_pages::testing::component::EventFixture;
use serde::Serialize;

#[derive(Serialize)]
struct ItemSelected {
    id: u64,
}

button.dispatch(
    EventFixture::custom("item-selected")
        .custom_detail(&ItemSelected { id: 42 }),
)?;
```

The absence of `.custom_detail(...)` is not the same as JSON `null`:
`EventFixture::custom("item-selected")` represents a same-named plain event,
whereas `.custom_detail_value(Value::Null)` represents the browser
`CustomEvent` default detail. Use `json!(...)` to supply malformed detail when
testing a structured decode error:

```rust,ignore
use serde_json::{Value, json};

let default_detail = EventFixture::custom("item-selected")
    .custom_detail_value(Value::Null);
let malformed_detail = EventFixture::custom("item-selected")
    .custom_detail_value(json!({ "id": "not-a-number" }));
```

Target-state setters include `value`, `checked`, `selected_values`, `files`,
and `content_editable`. Validation is atomic: an invalid compound target patch
does not apply its valid fields before returning `EventError::InvalidFixture`.
The error exposes `EventFixtureError` through `std::error::Error::source`.

Controlled `bind:` inputs share value normalization across initial rendering,
Signal updates, and input fixtures. This includes text sanitization, temporal
value validation, and range bounds and step constraints.

The originating `target()` and listener `current_target()` are distinct owned
snapshots. Bubbling creates a new current-target snapshot for each listener
while sharing propagation and default-prevention state.

## Async handlers

Typed async handlers schedule work on the screen-owned native scheduler. Call
`Screen::settle()` after dispatch to drain scheduled work and rerenders. The
method continues until tasks created by other tasks are also complete.

## Raw and component events

Use a standard `EventFixture` for intrinsic catalog events. Use
`EventFixture::custom` with `@custom("name")` for raw listeners, or add
`.custom_detail(...)` for typed `@custom::<Detail>("name")` listeners.
Component event props keep the argument type declared by the component prop;
they are not converted through the intrinsic event catalog.
