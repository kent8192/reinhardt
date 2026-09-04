# Reinhardt Pages for React developers

This guide maps common React frontend concepts to Reinhardt Pages. It is
written for developers who already know React, JSX, hooks, server actions, and
hydration, and want to transfer that mental model to a Rust + WASM Reinhardt
application.

## Quick mapping

| React concept | Reinhardt Pages concept | Main difference |
| --- | --- | --- |
| JSX | `page!` macro | Rust expressions and typed parameters are used inside the macro. |
| Function component | Rust function returning `Page` | Props are normal typed Rust arguments or structs. |
| Fragment | Multiple top-level `page!` nodes or `Page::fragment` | The output is a `Page::Fragment`, not a virtual DOM fragment. |
| `useState` | `use_state` returning `(Signal<T>, SetState<T>)` | Reads use `signal.get()`, writes use `set(value)` or `set.update(...)`. |
| `useEffect` | `use_effect(f, deps)` | Return `()` for no cleanup or `Option<C>` for cleanup; dependencies are explicit Rust tuples, for example `(count,)`. |
| `useLayoutEffect` | `use_layout_effect(f, deps)` | Same dependency model, layout timing. |
| `useMemo` | `use_memo(f, deps)` | Returns `Memo<T>`; read it with `.get()`. |
| `useCallback` | `use_callback(f, deps)` / `use_callback_with(f, deps)` | Returns a typed `Callback`, usually for event handlers. |
| `useReducer` | `use_reducer(reducer, initial)` | The reducer is a pure Rust function from `(&State, Action)` to `State`. |
| `useRef` | `use_ref(initial)` | Mutating a `Ref<T>` does not notify the reactive graph. |
| `useContext` | `Context<T>` + `provide_context` + `use_context` | Missing context returns `Option<T>`. |
| `useTransition` | `use_transition()` | Returns `TransitionState` with `is_pending` and `start_transition`. |
| `useDeferredValue` | `use_deferred_value(signal)` | Defers a `Signal<T>` value. |
| React actions / server functions | `use_action` + `#[server_fn]` | Server calls are typed Rust functions with generated WASM client stubs. |
| Action buttons and async result/resource panels | `reinhardt_pages::ui::{ActionButton, ActionResultPanel, ResourcePanel}` | Headless components bind typed action/resource state to slots; styling and accessible announcements remain application-owned. |

## React 19 and 19.2 parity classification

React 19 and 19.2 added frontend APIs around form actions, async reads,
server/client boundaries, metadata, asset loading, and transition boundaries.
Reinhardt Pages maps those expectations to Rust-first APIs instead of copying
React API names.

| React concept | Reinhardt classification | Tracking |
| --- | --- | --- |
| `useActionState` | `use_action_state` wraps `use_action` with lifecycle callbacks, dispatch helpers, and result/error rendering helpers; form validation remains explicit through `form!` / `use_form`. | #5548 |
| `<form action={function}>` | Explicit non-goal. Reinhardt keeps static form contracts and typed RPC bindings separate. | #5309 |
| Generic `use(...)` for Promise reads | Explicit non-goal. Use `use_resource(fetcher, deps)` for async data. | #5310 |
| Generic `use(...)` for Context reads | Explicit non-goal. Use typed `Context<T>` with `use_context`. | #5310 |
| Suspense integration for async reads | Existing API with different semantics through `SuspenseBoundary` and resource tracking. | #5310 |
| React Server Components and RSC/Flight transport | Explicit non-goal. Reinhardt does not provide React-compatible component transport. | #5311 |
| `"use client"` / `"use server"` directives | Explicit non-goal. Reinhardt uses Rust/WASM targets and `#[server_fn]`, not directive strings. | #5311 |
| Server reference passing | Explicit non-goal. `#[server_fn]` generates typed client stubs but does not serialize server function references into client components. | #5311 |
| Automatic metadata hoisting | Explicit non-goal. Reinhardt metadata stays explicit through `head!` and `Head`; component body nodes are not hoisted into the document head. | #5312 |
| React DOM asset APIs such as `preinit`, `preload`, `preconnect`, and `prefetchDNS` | Reinhardt-native explicit `Head` / `LinkTag` asset hint helpers with exact duplicate SSR deduplication; no browser-only imperative asset API. | #5312 |
| `createPortal` | Reinhardt-native explicit `Portal` / `mount_portal` API. `ClientLauncher::ensure_portal` remains only a launcher helper for idempotent body-level mounts. | #5313 |
| Custom element property, attribute, and event interop | Explicit DOM interop API. HTML attributes stay attributes; JS properties use `Element::set_property` / `get_property`, and custom event payloads use raw or typed `CustomEvent.detail` listeners. | #5314 |
| `ref` as a regular prop | Explicit non-goal. Reinhardt does not treat `ref` as a magic component prop; use typed props, `use_ref`, and explicit DOM handles. | #5314 |
| `Activity` and `ViewTransition` | Explicit boundary APIs. `ActivityBoundary` preserves hidden subtrees; `ViewTransitionBoundary` marks transition participants, and `start_view_transition` uses the browser API on WASM with a fallback. | #5315 |
| Cross-target API parity guardrails | Implementation follow-up for a wasm/server parity macro before broadening dual-target public APIs. | #5199 |

The umbrella tracker for this classification is #5198. It should close only
after this table reflects the final outcome of each focused follow-up.

## Metadata and asset loading

React 19 allows metadata such as `<title>`, `<meta>`, and `<link>` to appear in
component bodies and be hoisted into the document head. Reinhardt Pages keeps
that contract explicit. Document metadata belongs in `head!` or in a `Head`
value attached to the page; ordinary `page!` body nodes render where they are
declared.

Asset loading hints use the same explicit head model. `Head` and `LinkTag`
provide helpers for common browser hints:

- `preconnect`
- `dns_prefetch`
- `module_preload`
- `preload_script`
- `preload_style`
- `preload_image`
- `preload_font`

```rust,ignore
use reinhardt::pages::prelude::*;

fn document_head() -> Head {
    Head::new()
        .title("Dashboard")
        .preconnect("https://cdn.example.com")
        .dns_prefetch("https://cdn.example.com")
        .module_preload("/static/app.mjs")
        .preload_script("/static/app.js")
        .preload_style("/static/app.css")
        .preload_image("/static/hero.png")
        .preload_font("/static/font.woff2")
}
```

`preload_font` emits `crossorigin="anonymous"` by default so CSS `@font-face`
loads can reuse the preload instead of issuing a second font request.

During SSR, Reinhardt removes exact duplicate head entries after rendering each
entry to HTML, including duplicates between renderer defaults and a supplied
`Head`. The deduplication is conservative: entries with different attributes,
media conditions, `crossorigin` values, or Open Graph payloads remain separate.
Hydration does not scan component bodies for metadata or run a browser-only
imperative asset loader; the server-rendered head remains the deterministic
source of document-level metadata and hints.

## Components, props, and children

In React, a component is a function that returns JSX. In Reinhardt Pages, a
component is usually an ordinary Rust function that returns `Page`.

```rust,ignore
use reinhardt::pages::prelude::*;

#[derive(Clone)]
struct UserCardProps {
    name: String,
    role: String,
}

fn user_card(props: UserCardProps) -> Page {
    page!({
        article {
            class: "user-card",
            h2 { { props.name.clone() } }
            p { { props.role.clone() } }
        }
    })
}
```

Children are explicit values. Use a `Page` argument when the caller should pass
rendered children, or use `Page::fragment` when the component needs to group
multiple children without adding a wrapper.

```rust,ignore
use reinhardt::pages::prelude::*;

fn panel(title: String, body: Page) -> Page {
    page!({
        section {
            class: "panel",
            h2 { { title.clone() } }
            { { body.clone() } }
        }
    })
}
```

## JSX to `page!`

`page!` is the closest Reinhardt Pages equivalent to JSX. It produces `Page`
values that can be rendered on the server, hydrated in the browser, or composed
with other pages.

```rust,ignore
use reinhardt::pages::prelude::*;

fn counter_button(count: Signal<i32>, set_count: SetState<i32>) -> Page {
    page!({
        button {
            class: "counter",
            @click: {
                let set_count = set_count.clone();
                move |_event| set_count.update(|current| current + 1)
            },
            "Count: "
            { count.get().to_string() }
        }
    })
}
```

The syntax is intentionally Rust-first:

- Attribute names are Rust identifiers where possible, such as `class`.
- Event handlers use `@event_name`, such as `@click`.
- Rust expressions are written in braces.
- `page!({ ... })` is the usual form for functions that return a `Page`; free
  value identifiers from the surrounding scope are implicit captures and must
  implement `Clone`.
- `page!(|| { ... })` and `page!(|props: Props| { ... })` remain available for
  reusable factories that are called later. Closure forms keep strict capture
  discipline, so values used in the body must be declared as parameters.

### Event payloads

React's `SyntheticEvent` is one broad wrapper. Reinhardt selects one Rust
payload type for each standard intrinsic event. `@click` receives
`ClickEvent`, `@input` receives `InputEvent`, and capability methods exist only
where the catalog permits them. `target()` identifies the originating element;
`current_target()` is an owned listener-element snapshot and remains usable in
an async handler after an await.

```rust,ignore
use reinhardt_pages::event::{ClickEvent, InputEvent};
use reinhardt_pages::prelude::*;

page!({
    button { @click: |event: ClickEvent| {
        event.stop_propagation();
    }, "Stop" }
    input { @input: |event: InputEvent| {
        if let Ok(value) = event.value() {
            info_log!("{value}");
        }
    } }
})
```

Custom intrinsic events have adjacent raw and typed forms:

```rust,ignore
use reinhardt_pages::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct ItemSelected {
    id: u64,
}

page!({
    // Raw event transport for arbitrary DOM interop.
    button { @custom("item-selected"): |event: Event| { inspect(event); } }

    // Typed browser CustomEvent.detail decoding.
    button { @custom::<ItemSelected>("item-selected"): |event| {
        if let Ok(detail) = event.detail() {
            select(detail.id);
        }
    } }
})
```

`CustomEvent::detail()` borrows the cached decoded detail, while
`CustomEvent::into_detail()` consumes the event and returns the owned detail.
Decode failures are structured as `CustomEventDetailError::NotCustomEvent` for
a same-named plain event and `CustomEventDetailError::Deserialize` for an
invalid detail; the latter includes the event name and target Rust type. Its
decoder-provided message is not stable across native and WASM targets.

Component event props are not DOM events: their argument type comes from the
component's declared prop. Native component tests execute standard handlers
with `EventFixture`, including bubbling, target state, and async settling.

## Controlled and uncontrolled form controls

A control without `bind:` is uncontrolled: its current value belongs to the
DOM, and application code reads it through an event payload or another
explicit DOM integration. A control with `bind:` connects the DOM property to
a `Signal`, providing the Reinhardt equivalent of a React controlled input.

| Control shape | Bound signal |
| --- | --- |
| `input` with no `type` or static `type: "text"`, `"search"`, `"tel"`, `"url"`, `"email"`, `"password"`, `"color"`, `"date"`, `"datetime-local"`, `"month"`, `"week"`, or `"time"` | `Signal<String>` |
| `input` with static `type: "number"` or `"range"` | `Signal<T>` where `T: NumberValue`; optionally an error signal |
| `input` with static `type: "checkbox"` | `Signal<bool>` |
| `input` with static `type: "radio"` | `Signal<String>`; each radio also declares a static or dynamic `value` expression |
| `textarea` | `Signal<String>` |
| `select` with no `multiple` or static `multiple: false` | `Signal<String>` |
| `select` with static `multiple: true` | `Signal<Vec<String>>` |

Other input types, including `file`, are not binding shapes. The obsolete
`datetime` type is not an alias for `datetime-local`. Bound input `type` and
select `multiple` classifiers must be static so the macro can validate the
signal type at compile time.

```rust
use reinhardt_pages::prelude::*;

let query = Signal::new(String::new());
let parse_error = Signal::new(None::<NumberParseError>);
let amount = Signal::new(0_f64);

page!({
    input { aria_label: "Search", bind: query, placeholder: "Search" }
    input {
        aria_label: "Amount",
        type: "number",
        bind: number(amount, parse_error),
    }
})
```

The ownership transition during hydration is deliberate. The live DOM wins
initially: Reinhardt adopts browser-restored values and user edits made before
hydration instead of overwriting them with the server-time signal snapshot.
After hydration, the signal wins: application writes update the corresponding
DOM property. User input updates the signal before an explicit handler for the
same event runs, so the handler observes the new signal value. Password values
are written only to the live DOM property, never to SSR or the `value` content
attribute. Reactive attributes that cannot affect a control value also leave
an in-progress browser edit untouched.

Date/time inputs bind their browser serialization rather than a Rust date/time
type. Typical values are `2026-08-31`, `2026-08-31T10:30`, `2026-08`,
`2026-W36`, and `10:30`; `""` represents an empty control. User edits commit
the browser-normalized `HTMLInputElement.value`, so an invalid or incomplete
editor value is observed as `""`. Application writes should be a valid
serialization or `""`. Browser sanitization of an application write changes
the DOM property but does not silently rewrite the signal.

Text writes are deferred while an IME composition is active. The completed
value is committed at `compositionend`, and a duplicate final `input` event is
not committed twice. Explicit composition and input handlers still run in
normal dispatch order.

For numeric controls, `bind: number(value, error)` preserves the last valid
numeric signal and the user's raw text when parsing fails. The error signal is
cleared after a valid value and otherwise contains a `NumberParseError` with
one of these stable meanings:

- `Empty`: the browser-exposed value is empty.
- `Incomplete`: the text is a valid prefix such as `-`, `1.`, or `1e-`.
- `Invalid`: the text is not a numeric lexeme.
- `OutOfRange`: the number cannot be represented by the bound primitive.

Browsers may sanitize an incomplete HTML number value to an empty string before
the `input` handler runs. The binding tracks the editor's `beforeinput` changes
and unmodified Arrow/Home/End keyboard selection moves so recoverable incomplete
states remain available through `NumberParseError::raw`. Modifier-key commands,
including Ctrl/Cmd+A, and already-canceled key events are treated as unknown
positions.
Number inputs do not expose selection ranges,
and `beforeinput` target ranges are empty. If a pointer moves the caret and the
very next edit is sanitized, the position is unknowable; the binding reports
the browser's empty value rather than fabricating raw text. A subsequent valid
input resynchronizes the predicted caret. Composition updates remain deferred
until `compositionend`, and the duplicate final `input` event is not committed
twice.

Use an explicit typed handler when the binding is not the only response to an
event. For browser-specific integrations that truly need the underlying DOM
event, use `payload.raw()` on WASM or wrap a low-level handler with
`raw_event_handler`. Keep `bind:` responsible for synchronization; the raw
event path is an escape hatch, not a second source of control state.

Imperative DOM lookup is therefore unnecessary for ordinary controlled input:

```rust,ignore
// Before: WASM-only DOM ownership.
let input = document
    .get_element_by_id("search")
    .unwrap()
    .dyn_into::<web_sys::HtmlInputElement>()
    .unwrap();
let query = input.value();

// After: cross-target signal ownership.
let query = Signal::new(String::new());
let query_for_submit = query.clone();
page!({
    input { id: "search", aria_label: "Search", bind: query }
    button {
        @click: move |_| submit(query_for_submit.get()),
        "Search"
    }
})
```

## State and reactivity

React state is component-local and re-rendered through the virtual DOM.
Reinhardt state is fine-grained: `Signal<T>` tracks readers and notifies only
the dependent reactive work. Use `SetState<T>` as a callable setter for direct
replacement, or the `SetStateExt::update` method when the next value depends on
the previous one.

```rust,ignore
use reinhardt::pages::prelude::*;

fn counter() -> Page {
    let (count, set_count) = use_state(0);
    counter_button(count, set_count)
}
```

Expression, `if`, and `for` nodes inside `page!` are auto-wrapped in reactive
render scopes. Read signals inside the page body when a branch should
re-evaluate as signals change. Values extracted before `page!` are static
snapshots.

```rust,ignore
page!({
    if count.get() == 0 {
        p { "No clicks yet" }
    } else {
        p { { format!("Clicked {}", count.get()) } }
    }
})
```

Reactive handles such as `Signal<T>`, `Memo<T>`, `Action<T, E>`,
`Resource<T, E>`, and `Callback<A, R>` are `Copy`. Pass them directly into
closures and dependency tuples. Non-reactive handles such as setter functions
may still need ordinary Rust cloning when they are reference-counted function
values.

## Effects and dependency tuples

React uses dependency arrays. Reinhardt Pages uses explicit dependency tuples.
The tuple is part of the Rust call, not inferred from signal reads inside the
effect body.

```rust,ignore
use reinhardt::pages::prelude::*;

let (count, _set_count) = use_state(0);

use_effect(
    move || {
        log::info!("count = {}", count.get());
    },
    (count,),
);
```

Important differences from React:

- Pass `()` for mount-only effects.
- Pass `(signal,)` for one dependency. The trailing comma matters.
- Reading a signal inside `use_effect`, `use_layout_effect`,
  `use_memo`, or `use_callback` does not create hidden subscriptions.
  Subscriptions come from the dependency tuple.
- Cleanup is returned as `Option<C>` from the closure, matching React's
  cleanup behavior in a Rust type.

## Memoization and callbacks

Use `use_memo` for derived values and `use_callback` or `use_callback_with` for
stable callback handles.

```rust,ignore
use reinhardt::pages::prelude::*;

let (items, _set_items) = use_state(vec![1, 2, 3, 4]);
let (threshold, _set_threshold) = use_state(2);

let visible = use_memo(
    move || {
        items
            .get()
            .into_iter()
            .filter(|item| *item > threshold.get())
            .collect::<Vec<_>>()
    },
    (items, threshold),
);

let on_click = use_callback(
    move |_event| {
        log::info!("visible item count = {}", visible.get().len());
    },
    (visible,),
);

let upload_click = use_callback(
    move |_| {
        index_action.reset();
        search_action.reset();
        upload_action.dispatch(route_project_id.get());
    },
    (route_project_id,),
);
```

## Reducers, refs, and context

`use_reducer` is useful when the next state depends on an action.

```rust,ignore
use reinhardt::pages::prelude::*;

#[derive(Clone)]
struct CounterState {
    value: i32,
}

enum CounterAction {
    Increment,
    Reset,
}

fn reducer(state: &CounterState, action: CounterAction) -> CounterState {
    match action {
        CounterAction::Increment => CounterState {
            value: state.value + 1,
        },
        CounterAction::Reset => CounterState { value: 0 },
    }
}

let (state, dispatch) = use_reducer(reducer, CounterState { value: 0 });
dispatch(CounterAction::Increment);
```

Use `use_ref` for mutable values that should not trigger reactive updates, such
as timers, previous values, or DOM handles.

```rust,ignore
let render_count = use_ref(0);
render_count.update(|count| *count += 1);
```

For custom elements, keep HTML attributes and JavaScript properties explicit.
`page!` attributes and `Element::set_attribute` render string attributes.
Use the DOM wrapper for property-based web component APIs after an element is
available on the client.

```rust,ignore
use reinhardt::pages::prelude::*;
use wasm_bindgen::JsValue;

let widget: Element = /* created or queried on WASM */;
widget.set_attribute("data-theme", "dark")?;
widget.set_property("value", &JsValue::from_str("selected"))?;
```

Custom element events use normal DOM event listener handles. Use
`add_custom_event_listener` for raw `JsValue` payloads, or
`add_typed_custom_event_listener` when `CustomEvent.detail` should deserialize
into a Rust type. The typed callback now receives the complete event rather
than a `Result` detail value:

```rust,ignore
use reinhardt_pages::prelude::CustomEvent;

// Before
|detail: Result<ItemSelected, String>| match detail {
    Ok(detail) => consume(detail),
    Err(error) => report(error),
}

// After
|event: CustomEvent<ItemSelected>| match event.into_detail() {
    Ok(detail) => consume(detail),
    Err(error) => report(error),
}
```

Use `detail()` instead when the handler should retain the event and borrow the
cached decoded detail. The error is a `CustomEventDetailError`, so callers can
match `NotCustomEvent` and `Deserialize` instead of parsing a string; the
decoder-specific `Deserialize::message` is not cross-target stable.

`CustomEvent::raw()` retains the original platform event for low-level DOM
interop. On WASM it is a `web_sys::Event`, including for typed listeners:

```rust,ignore
let handle = widget.add_typed_custom_event_listener("widget-change", |event| {
    let raw_event: &web_sys::Event = event.raw();
    inspect_browser_event(raw_event);
    if let Ok(detail) = event.detail() {
        consume(detail);
    }
});
```

Keep the returned handle alive while the listener should remain active.

`ref` is not a special prop in Reinhardt components. Pass explicit typed props
or callbacks when a component should expose behavior. Store mutable values in
`use_ref`; a future element-ref binding API would need a separate lifecycle
contract rather than React-style `ref` prop semantics.

Context is type-safe. A missing provider returns `None` instead of throwing.

```rust,ignore
use reinhardt::pages::prelude::*;

static THEME: Context<String> = create_context();

provide_context(&THEME, "dark".to_string());
let theme = use_context(&THEME).unwrap_or_else(|| "light".to_string());
```

## Transitions and deferred values

`use_transition` marks state updates as lower priority. On WASM, transition
work runs asynchronously; on native SSR it runs synchronously.

```rust,ignore
let transition = use_transition();
let (query, set_query) = use_state(String::new());

transition.start_transition({
    let set_query = set_query.clone();
    move || set_query("rust".to_string())
});

let deferred_query = use_deferred_value(query.clone());
```

Render loading or stale-state UI by reading `transition.is_pending.get()` and
`deferred_query.get()`.

`ActivityBoundary` is the state-preserved hiding primitive. It always renders
its content, including on SSR, and hides the wrapper with `hidden` /
`aria-hidden` when the mode is hidden. Use it when DOM-owned state should stay
mounted while the region is not visible.

```rust,ignore
let details_open = use_state(|| false);

let details = ActivityBoundary::default()
    .visible_when(details_open.0.get())
    .content(|| page!({
        section {
            h2 { "Details" }
            p { "The subtree stays rendered while hidden." }
        }
    }));
```

`ViewTransitionBoundary` is an SSR-safe marker for elements that should
participate in browser view transitions. It emits stable `data-rh-*` markers
and, when named, a CSS `view-transition-name`; it does not change hydration
semantics by itself. Names are normalized before they are written to inline CSS
so dynamic ids or slugs cannot inject style declarations.

```rust,ignore
let card = ViewTransitionBoundary::new()
    .name("selected-card")
    .content(|| page!({
        article { "Selected" }
    }));
```

On WASM, wrap state changes with `start_view_transition` when the browser
should coordinate a View Transition API update. Browsers without
`document.startViewTransition`, and native SSR, execute the update normally and
report an unsupported status.

```rust,ignore
let handle = start_view_transition(move || {
    selected_id.set(Some(id));
});

if handle.is_unsupported() {
    // The state update still ran; only browser transition support was missing.
}
```

## Forms, actions, and `#[server_fn]`

React server actions and client actions map to separate pieces in Reinhardt
Pages:

- `form!` defines a static form contract, including field metadata,
  validation, and rendered form structure.
- `use_form(&form)` builds runtime form state such as field values, validation
  errors, submit state, and submit lifecycle callbacks.
- `#[server_fn]` defines the server operation and generates the WASM client
  stub.
- `use_action` wraps an async mutation and exposes `Idle`, `Pending`,
  `Success`, and `Error` phases.
- `use_action_state` builds the same action handle with success/error
  callbacks, optional reset-on-success behavior, and dispatch callbacks for
  UI event handlers.
- `reinhardt_pages::ui::ActionButton` renders a semantic button and prevents a
  second dispatch while its `Action` is pending.
- `reinhardt_pages::ui::ActionResultPanel` maps action phases to repeatable
  `Fn() -> Page`, `Fn(&T) -> Page`, and `Fn(&E) -> Page` slots.
- `reinhardt_pages::ui::ResourcePanel` maps loading, empty, success, and error
  states to slots and can read `Resource::latest_after(action)` values.

React `useActionState` combines form submission, pending state, result state,
and errors behind one hook. Reinhardt keeps form validation explicit: use
`use_form` for typed form state and validation, then use `use_action_state`
or `use_action` to run the `#[server_fn]` mutation after the form is valid.
React's DOM `action={function}` behavior is not supported directly.

The UI primitives are intentionally headless. Applications own CSS and other
visual presentation, live-region roles and announcements, localization, and
redaction of errors before they are shown. Error slots receive typed `&E`
values, so an application can select a safe user-facing message instead of
exposing an internal diagnostic.

```rust,ignore
use reinhardt_pages::component::Page;
use reinhardt_pages::ui::{ActionButton, ActionResultPanel, ResourcePanel};

let save_button = ActionButton::new(save, project_id, Page::text("Save"));
let save_result = ActionResultPanel::new(save)
    .pending(|| Page::text("Saving"))
    .success(|value| Page::text(value.clone()));
let project_view = ResourcePanel::new(project)
    .loading(|| Page::text("Loading"))
    .success(|value| Page::text(value.clone()));
```

`ActionButton::new` takes an `Action<T, E>`, a cloneable payload `P`, and
`C: IntoPage`; `ActionButton::new_with` instead takes `F: Fn() -> P + 'static`
to build the payload at click time. `ActionResultPanel` and `ResourcePanel`
take repeatable closures, so their slot signatures remain explicit and do not
consume the values they render. See the `reinhardt_pages::ui` rustdoc for the
complete constructor and builder signatures.

Validated generated forms use `FormActionButton` rather than `ActionButton`.
Attach `FormAction::submit_handler()` to the containing form so button clicks
and Enter-key submission both run generated validation before dispatch.
`FormActionResultPanel` provides a separate `validation_error` slot alongside
typed mutation `error` and `success` slots. Use
`Resource::latest_after_form(&save)` for resource reconciliation. The raw
action remains private, so form validation cannot be bypassed through direct
payload dispatch.

```rust,ignore
use reinhardt::pages::prelude::*;
use reinhardt::pages::server_fn::{ServerFnError, server_fn};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Todo {
    id: u64,
    title: String,
}

#[server_fn]
pub async fn create_todo(title: String) -> Result<Todo, ServerFnError> {
    // Persist on the server.
    Ok(Todo { id: 1, title })
}

fn todo_form() -> Page {
    let create = use_action_state(|title: String| async move {
        create_todo(title).await.map_err(|error| error.to_string())
    })
    .on_success(|todo| {
        log::info!("created todo {}", todo.id);
    })
    .build();

    page!({
        button {
            disabled: create.is_pending(),
            @click: create.dispatching("Write docs".to_string()),
            "Create"
        }
        if create.last_result().is_some() {
            p {
                role: "status",
                "Todo created"
            }
        }
        if create.last_error().is_some() {
            p {
                role: "alert",
                { create.last_error().unwrap_or_default() }
            }
        }
    })
}
```

## Keyed queries and invalidating actions

React Query and SWR patterns map to `use_query` for reads and `use_action` for
mutations. `ClientLauncher` creates and owns one `QueryClient` for a browser
application. Every SSR request and every native component-test screen creates
an isolated client, so data and in-flight requests do not leak between users or
tests.

Set application-wide freshness and retention defaults on the launcher:

```rust,ignore
ClientLauncher::new("#root")
    .query_defaults(
        QueryDefaults::new()
            .stale_time(Duration::from_secs(30))
            .gc_time(Duration::from_secs(300)),
    )
    .router(app_router)
    .launch()?;
```

The server-function macro emits three typed helpers:

- `family()` identifies every argument set for one endpoint.
- `key(args...)` identifies one exact cached argument set.
- `query(args...)` pairs that key with the generated fetcher.

The cache ID is derived from the generated marker metadata and a SHA-256 digest
of canonical JSON arguments. The fetcher and key therefore cannot drift into
unrelated strings, raw arguments do not appear in hydration keys, and logically
equivalent object arguments share the same cache entry.

```rust,ignore
use std::time::Duration;

use reinhardt::pages::prelude::*;
use reinhardt::pages::server_fn::{ServerFnError, server_fn};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct JobSnapshot {
    id: i64,
    status: String,
}

#[server_fn]
pub async fn list_project_jobs(project_id: i64) -> Result<Vec<JobSnapshot>, ServerFnError> {
    Ok(Vec::new())
}

#[server_fn]
pub async fn retry_job(project_id: i64, job_id: i64) -> Result<(), ServerFnError> {
    Ok(())
}

fn jobs_panel(project_id: i64, failed_job_id: i64) -> Page {
    let jobs = use_query(
        list_project_jobs::query(project_id),
        QueryOptions::new().refetch_interval(Duration::from_secs(5)),
    );

    let client = queries();
    let retry = use_action(move |job_id: i64| {
        let client = client.clone();
        async move {
            let result = retry_job(project_id, job_id).await?;
            client.invalidate_family(list_project_jobs::family());
            Ok::<_, ServerFnError>(result)
        }
    });

    page!({
        button {
            disabled: retry.is_pending(),
            @click: retry.dispatching(failed_job_id),
            "Retry"
        }
        match jobs.snapshot().status {
            QueryStatus::Idle => p { role: "status", "Not requested" },
            QueryStatus::Pending => p { role: "status", "Loading" },
            QueryStatus::Success => p {
                { format!("{} jobs", jobs.data().unwrap_or_default().len()) }
            },
            QueryStatus::Error => p { role: "alert", "Could not load jobs" },
        }
    })
}
```

Use exact invalidation when only one argument set changed:

```rust,ignore
client.invalidate(&list_project_jobs::key(project_id));
```

Use family invalidation when the mutation can affect several project filters or
pages:

```rust,ignore
client.invalidate_family(list_project_jobs::family());
```

Keep invalidation after the mutation's `await?`, as shown above, so a failed
action does not refetch unchanged server state.

For reads that are not server functions, define a manual family. The family
owns identity and argument types; each descriptor supplies its fetcher:

```rust,ignore
const AUDIT_EVENTS: QueryFamily<u64, Vec<AuditEvent>, ApiError> =
    QueryFamily::new("audit-events.by-project");

let events = use_query(
    AUDIT_EVENTS.query(project_id, move || fetch_audit_events(project_id)),
    QueryOptions::new()
        .enabled(can_view_audit)
        .stale_time(Duration::from_secs(60))
        .gc_time(Duration::from_secs(600)),
);
```

The manual family ID and `Args` encoding are a persistent cache contract.
Descriptors that reuse the same family ID and argument type must describe the
same semantic operation and serialize arguments to the same canonical JSON
shape. Version the ID, for example `audit-events.by-project.v2`, whenever the
operation meaning or canonical encoding changes, even if the Rust types do not.

`QueryOptions` belong to the mounted observer. `enabled(false)` with no cached
result reports `QueryStatus::Idle`; an enabled initial request reports
`Pending`, followed by `Success` or `Error`. A background refetch preserves
successful `data`, sets `is_fetching`, and reports any failure through
`refetch_error`; it does not replace the screen with an initial-load error.
`is_stale` is observer-specific because observers may choose different
`stale_time` values for the same shared entry.

`QueryHandle` implements the same Suspense tracking interface as `Resource`, so
`SuspenseBoundary::track(jobs.clone())` can associate a keyed query with the
boundary for SSR streaming and native component tests. Queries keep prior
successful data visible during background refetches; use `is_fetching()` when a
UI needs to distinguish refresh work from the initial pending state.

Polling is observer-owned through `QueryOptions::refetch_interval`. Reinhardt
suspends polling while `document.visibilityState` is hidden. When the document
becomes visible, stale data refetches immediately and fresh data waits for its
next interval.

Retry policy is also observer-owned, but observers for the same key coordinate
one entry-level attempt sequence. Intermediate errors stay private until the
sequence is exhausted, and `is_fetching` is false during backoff. Browser
backoff pauses while the document is hidden. On visibility resume, stale data
retries immediately while fresh data waits only the saved remaining delay.

During SSR, one request-local client deduplicates query reads from route loaders
and components. Only settled snapshots are serialized. Hydration seeds the
browser application's client before its first observer mounts, so matching
generated keys reuse server data without a duplicate initial request.

SSR retry requires both a query policy and an explicit renderer gate. The
resource timeout is one budget covering fetch attempts, backoff, and jitter:

```rust,ignore
let options = SsrOptions::new()
    .query_retries(true)
    .resource_timeout(Duration::from_secs(2));
```

### Migrating to query client v2

Move the fetcher into a descriptor and observer policy into `QueryOptions`:

```rust,ignore
use reinhardt_pages::server_fn::{ServerFnError, ServerFnErrorKind};

// Before
let jobs = use_query(list_project_jobs::key(project_id)).poll(Duration::from_secs(5));

// After
let jobs = use_query(
    list_project_jobs::query(project_id),
    QueryOptions::new().refetch_interval(Duration::from_secs(5)),
);

let retrying_jobs = use_query(
    list_project_jobs::query(project_id),
    QueryOptions::new().retry(
        RetryPolicy::exponential()
            .max_attempts(3)
            .base_delay(Duration::from_millis(250))
            .max_delay(Duration::from_secs(5))
            .jitter(true)
            .when(|error: &ServerFnError| {
                matches!(
                    error.kind(),
                    ServerFnErrorKind::Server | ServerFnErrorKind::Transport
                )
            }),
    ),
);
```

`max_attempts(3)` means the initial request plus at most two retries. Equal
jitter remains between half and all of the nominal delay.

The following before-only APIs were removed:

- Before: `QueryKey::new(...)`. After: generated `family`, `key`, and `query`
  helpers, or a manual `QueryFamily`.
- Before: query-handle `.poll(...)`, `.stale_time(...)`, and `.gc_time(...)`.
  After: mount-time `QueryOptions`.
- Before: `use_mutation(...)`. After: `use_action(...)`.
- Before: `Action::invalidates(...)`. After: call exact or family invalidation
  explicitly after mutation success.

Install retry behavior with `QueryOptions::retry`. Entity normalization (#5843)
remains an explicit non-goal for query client v2.

For generated forms, read submit state from the runtime returned by `use_form`:

```rust,ignore
let runtime = use_form(&profile_form)
    .on_submit_start(|handle| {
        log::info!("submitting {:?}", handle.get_values());
    })
    .on_submit_success(|handle| {
        assert!(handle.form_state().is_submit_successful.get());
    })
    .build();

let state = runtime.form_state();
let pending = state.is_submitting.get();
let submit_succeeded = state.is_submit_successful.get();
let visible_error = state.error.get();
```

For optimistic UI, keep predicted state in `use_optimistic`, attach it with
`Action::with_optimistic`, and let the action confirm it on success or revert it
on error.

## Routing

React Router concepts map to `ClientRouter`, route handlers returning `Page`,
`Link` for anchor generation, and `use_router` for imperative navigation.

```rust,ignore
use reinhardt::pages::prelude::*;
use reinhardt::ClientRouter;

fn home() -> Page {
    page!({ h1 { "Home" } })
}

fn app_router() -> ClientRouter {
    ClientRouter::new()
        .route("home", "/", home)
        .not_found(|| page!({ h1 { "Not found" } }))
}
```

## Portals

React `createPortal` renders children into another DOM node while keeping the
component relationship in React's virtual tree. Reinhardt Pages uses an explicit
portal mount API instead of treating `ClientLauncher::ensure_portal` as a
general primitive.

Use `Portal` or `mount_portal` when a `Page` should render into an existing DOM
target such as a modal root or toast root:

```rust,ignore
use reinhardt::pages::prelude::*;

fn open_dialog() -> Result<PortalHandle, PortalError> {
    mount_portal(
        PortalTarget::element_id("modal-root"),
        page!({
            div {
                role: "dialog",
                "Dialog content"
            }
        }),
    )
}
```

The returned `PortalHandle` owns the mounted host. Dropping it removes the
portal host from the target, so callers should keep the handle in the same
lifetime scope as the source view or effect that opened the portal.

SSR is explicit: portal children are not duplicated into the source tree. Use
`Portal::placeholder()` when the server output should include a deterministic
`<template data-rh-portal="...">` marker. Hydration does not move server
nodes across the document; WASM `mount_portal` mounts the `Page` into the target
and attaches event handlers through the normal `PageExt::mount` path.

## SSR and hydration

React hydrates server HTML into a virtual DOM tree. Reinhardt Pages renders
`Page` values on the server and hydrates the existing DOM with fine-grained
reactive markers and event handlers. The important mental shift is that most
updates flow through signals and reactive scopes, not a full component-tree
diff.

Practical consequences:

- Keep server-rendered markup deterministic for the initial state.
- Put client-only effects in `use_effect` or `use_layout_effect`, not in the
  server render path.
- Use `#[server_fn]` for typed client-to-server mutations instead of manually
  duplicating API request stubs.
- Treat `#[server_fn]` as typed RPC, not as React Server Actions reference
  serialization.
- Prefer signal reads inside `page!` expression, `if`, and `for` nodes for
  reactive view branches.

## Intentional differences from React

Reinhardt Pages is React-aligned, not React-compatible. These differences are
intentional:

- There is no React-style indexed hook slot list. Hooks are Rust functions that
  return handles such as `Signal<T>`, `Memo<T>`, `Ref<T>`, and `Action<T, E>`,
  so there is no hook-call-order rule for preserving slot identity. Still,
  create long-lived state at component construction time instead of inside
  frequently re-run reactive page branches unless new state is intended.
- Effect, memo, and callback dependencies are explicit tuples, not arrays and
  not implicit captures.
- Updates are fine-grained through signals instead of virtual DOM diffing.
- Event and DOM APIs are typed Rust APIs over `web-sys` on WASM and owned event
  snapshots on native; native component tests can execute the same handlers.
- Missing context is represented as `Option<T>`.
- There is no catch-all React-style `use(...)` API. Async resource reads,
  context reads, and loading boundaries use separate typed APIs.
- There is no React-compatible RSC/Flight transport or directive-string
  boundary. Use `#[server_fn]` for client-to-server RPC.

## Migration checklist

When porting a React component:

1. Convert JSX markup to `page!`.
2. Convert props to typed Rust arguments or a `Props` struct.
3. Replace `useState` values with `Signal<T>` reads and `SetState<T>` writes.
4. Replace dependency arrays with dependency tuples.
5. Move async mutations behind `#[server_fn]` and trigger them with
   `use_action`.
6. Replace React Router route elements with `ClientRouter` route handlers.
7. Review SSR assumptions and keep browser-only work inside effects.
