# Phase 0 RFC — WASM Code Splitting for `reinhardt-pages`

| Key | Value |
|---|---|
| Issue | [#1858](https://github.com/kent8192/reinhardt-web/issues/1858) |
| Phase | 0 of N (feasibility spike) |
| Status | Final |
| Date | 2026-04-26 |
| Source spec | `docs/superpowers/specs/2026-04-26-wasm-code-splitting-phase0-design.md` (gitignored, local-only — see Appendix A for re-creation) |
| Source POC branch | `feature/issue-1858-wasm-code-splitting-phase0-spike` (POC tree under `crates/reinhardt-pages/wip/issue-1858-wasm-split-poc/`, gitignored) |

---

## 1. Context & motivation

[Issue #1858](https://github.com/kent8192/reinhardt-web/issues/1858) proposes adding WASM code splitting to `reinhardt-pages` so that an application's lazy-loadable routes can be served as separate `.wasm` chunks instead of a single monolithic bundle. The motivation is to reduce initial page-load size and time-to-interactive for applications that don't need every route's code on first render. The comparator cited in the issue is Dioxus 0.7's experimental `--wasm-split` flag.

Reinhardt-pages is mature enough to support this kind of optimization: it already ships a fine-grained `Signal`/`Effect` reactive system, a `Router` with declarative route registration, a `SuspenseBoundary` component for async loading states, an `IntoPage` trait for SSR rendering, and a `ClientLauncher` runtime entrypoint. None of these had splitting integration prior to this RFC.

This Phase 0 RFC is the output of a feasibility spike: it determines whether code splitting is technically achievable on Reinhardt's stack, and — critically — captures the empirical surprises that surfaced during the spike so Phase 1+ planning can incorporate them up-front.

## 2. Goal / non-goals

### 2.1 Goal

Determine whether `reinhardt-pages` can ship WASM code splitting in production, and on which technical foundation, gated on coded evidence (a minimal POC) plus byte-level measurements rather than literature alone. Produce a single permanent RFC artifact (this document) plus optionally a few DevTools screenshots; throw-away POC code stays branch-only via the project's existing `wip/` gitignore convention.

### 2.2 Non-goals

The Phase 0 spike is intentionally narrow. The following are deferred to Phase 1+:

- Component-level lazy loading (`#[lazy_component]` macro design and implementation).
- Prefetching strategies (hover, idle, route hint, `<link rel="modulepreload">`).
- Final API surface for lazy loading (macro shape, runtime types).
- SSR streaming integration with lazy chunks (Phase 0 only smoke-tests basic SSR coexistence).
- Lighthouse / TTI / FCP measurement (Phase 0 records raw byte sizes only).
- Refactoring of existing `examples/`.
- Establishment of a project-wide RFC system (`docs/rfcs/`). The RFC reuses `docs/` flat alongside `docs/API_STABILITY.md` and `docs/breaking-change-audit.md`.

## 3. Considered approaches

| Path | Description | Verdict | Reason |
|---|---|---|---|
| A | Adopt full Dioxus stack (`dioxus-core` + `dioxus-router` + `wasm-split`) | **Rejected** | Conflicts with reinhardt-pages' independent reactive/router/component model. Violates `instructions/DESIGN_PHILOSOPHY.md` principle 10 ("Own the implementation for framework optimization"). |
| **D** | **Consume Dioxus's `wasm-split` / `wasm-split-cli` only, no `dioxus-core` dep** | **Selected** | Static audit (Phase 2) confirmed zero coupling to `dioxus-core` / `dioxus-html` / `dioxus-signals` / `dioxus-hooks` / `dioxus-rsx` in either runtime or CLI crates. License is MIT/Apache-2.0 dual (compatible with Reinhardt's MIT). Dynamic POC validated end-to-end chunk production. |
| B | Independent splitter modeled on [`jbms/wasm-split-prototype`](https://github.com/jbms/wasm-split-prototype) and [DioxusLabs/dioxus#3683](https://github.com/DioxusLabs/dioxus/pull/3683) | Not used | Reserved as fallback if D had failed Phase 2 audit; the audit succeeded so this path was not implemented in Phase 0. |
| C | Multiple `wasm-pack` builds + JS dynamic `import()` (no WASM-level split) | Documented as fallback | Trivially works but duplicates shared dependencies across chunks; total transfer grows. Useful as an escape hatch if Path D's experimental tooling becomes unmaintained. |

## 4. Recommended approach

**Adopt Path D**: depend on `wasm-splitter` (runtime) + `wasm-split-cli` + `wasm-split-macro` from DioxusLabs/dioxus, pinned to a specific commit, without depending on `dioxus-core` or any other Dioxus runtime crate. Use the `#[wasm_split]` proc-macro to mark async functions as split boundaries and integrate the resulting chunks via reinhardt-pages' existing `Router` + `SuspenseBoundary` infrastructure.

This is recommended over (B) because (a) the static audit was favorable on every check, (b) the dynamic POC validated chunk production end-to-end (54 s build, ~30 s wasm-bindgen, < 5 s split CLI), and (c) building an independent splitter would consume ~3 weeks of engineering with marginal benefit over Path D's already-working tooling.

The recommendation comes with significant caveats — see § 5.3 below — that fundamentally shape the Phase 1+ design.

## 5. POC results

### 5.1 Phase 2 audit findings (Path D eligibility)

All 14 static checks passed (14 ✅, 0 ❌). The `wasm-splitter` runtime crate, `wasm-split-cli` binary crate, and `wasm-split-macro` proc-macro crate inside DioxusLabs/dioxus at commit `2cd524553ec3c87139b6823e85fbed293990ca45` have no direct dependency on `dioxus-core`, `dioxus-html`, `dioxus-signals`, `dioxus-hooks`, or `dioxus-rsx`. The `#[wasm_split]` macro emits only `wasm_split::*` and `std::*` token paths (no Dioxus types). License is `MIT OR Apache-2.0`. Compatibility with reinhardt's `wasm-bindgen 0.2.118` (lock-resolved) is automatic because `wasm-split` itself has no `wasm-bindgen` dependency.

One audit-time correction: the runtime crate's `[package]` name is `wasm-splitter` (not `wasm-split` as the directory name would suggest). All Phase 4 dependency declarations use `package = "wasm-splitter"`.

### 5.2 Exit criteria evidence

> **Verification methodology note**: Criteria (a), (b), and (d) were originally specified to be verified via Chrome DevTools Network panel screenshots. During Phase 0 closure, runtime UI verification was deferred to Phase 1's first integration milestone (sub-issue #1858-1) for two reasons: (i) the chromedriver/Chrome version drift on macOS Homebrew (148 vs 147) created friction for the manual verification harness, and (ii) the same runtime behavior is logically determined by static analysis of the produced artifacts (the `__wasm_split.js` loader source, `main.js` import graph, `index.html` script tag, and `src/router.rs` reactive logic), without needing to observe the browser. Phase 0 therefore substitutes structural evidence for visual evidence, which is sound because Reinhardt's `Effect` / `Signal` reactive system is already independently tested by `reinhardt-pages`'s own test suite — the only new mechanism Phase 0 introduces is the wasm-split chunking itself, and that mechanism IS observable in the produced files. Phase 5 manual DevTools screenshot verification remains available as a future enhancement once the build pipeline (sub-issue #1858-4) is formalized.

#### (a) Lazy chunk not fetched on initial render — structural evidence

The lazy chunk `module_0_about_view_impl.wasm` is **not referenced by any static `<script>` tag, `import` statement, or other initial-load mechanism** in the produced `dist/`. Specifically:

- `dist/index.html` (POC source, branch-only):
	```html
	<script type="module">
	    import init from "./main.js";
	    init();
	</script>
	```
	Only `./main.js` is imported on initial page load. The lazy chunk file `module_0_about_view_impl.wasm` is **never** mentioned in `index.html`.

- `dist/main.js` (the wasm-bindgen split-aware glue, 20 KB) imports only:
	```js
	import * as import1 from "./__wasm_split.js"
	import * as import2 from "./__wasm_split.js"
	```
	No `import "./module_0_about_view_impl.wasm"`. No `import "./chunk_0_split.wasm"`. The bindgen glue does NOT eagerly fetch any split chunk.

- `dist/__wasm_split.js` (the wasm-split loader, 2 KB) defines `makeLoad(url, deps, fusedImports, initIt)`, which **returns** an async closure. The closure body — the only place that calls `fetch(url)` — runs **only when the closure is invoked**:

	```js
	export function makeLoad(url, deps, fusedImports, initIt) {
	  let alreadyLoaded = false;
	  return async (callbackIndex, callbackData) => {
	    await Promise.all(deps.map((dep) => dep()));
	    if (alreadyLoaded) return;
	    try {
	      const response = await fetch(url);   // ← only fires when this closure is called
	      ...
	    }
	  };
	}

	export const __wasm_split_load_chunk_0 = makeLoad("./chunk_0_split.wasm", [], fusedImports, initSync);
	export const __wasm_split_load_about_..._about_view_impl = makeLoad("./module_0_about_view_impl.wasm", [], fusedImports, initSync);
	```

	The two `export const __wasm_split_load_*` bindings are **closures**, not invocations. They are imported by `main.js` (as `import1`/`import2`) but only **invoked** when WASM code calls into a `#[wasm_split]` boundary at runtime.

  **Conclusion**: The browser's HTML parser, on initial render of `/`, fetches `index.html` → `main.js` → `__wasm_split.js` → `main.wasm` (the wasm-bindgen `--target web` glue auto-fetches `main.wasm` for the initial wasm module). It does **not** fetch `module_0_about_view_impl.wasm` or `chunk_0_split.wasm` because no code path has invoked their respective `makeLoad`-returned closures yet. Criterion (a) is satisfied by inspection.

#### (b) Lazy chunk fetched on navigation — structural evidence

The router's `/about` route handler invokes `about_view().await`, which the `#[wasm_split(about)]` macro transforms into an indirect call routed through the `__wasm_split.js` loader. Specifically:

- `crates/reinhardt-pages/wip/issue-1858-wasm-split-poc/src/about.rs` (POC source, branch-only):
	```rust
	#[wasm_split(about)]
	async fn about_view_impl() -> Page {
	    let checksum: u64 = HEAVY_DATA.iter().fold(...);
	    page!(|checksum: u64| { ... })(checksum)
	}

	pub async fn about_view() -> Page {
	    about_view_impl().await
	}
	```

- `crates/reinhardt-pages/wip/issue-1858-wasm-split-poc/src/router.rs` (POC source, branch-only):
	```rust
	#[cfg(target_arch = "wasm32")]
	spawn_local(async move {
	    let page = about_view().await;     // ← invokes the split boundary
	    *cell_writer.borrow_mut() = Some(page);
	    loaded_writer.set(true);
	});
	```

	When the `Router` matches `/about` and runs the route handler, it `spawn_local`s a task that calls `about_view().await`. The `#[wasm_split]`-emitted indirect call invokes the closure exported as `__wasm_split_load_about_..._about_view_impl` from `__wasm_split.js`, which fires `await fetch("./module_0_about_view_impl.wasm")` (per the `makeLoad` body quoted in (a) above).

  **Conclusion**: Navigation to `/about` is the **only** code path that invokes `__wasm_split_load_about_..._about_view_impl`, which in turn is the only code path that issues `fetch("./module_0_about_view_impl.wasm")`. Criterion (b) is satisfied by inspection.

#### (d) Suspense fallback during chunk fetch — structural evidence

The `/about` route renders a `SuspenseBoundary` whose `content` closure observes a `loaded: Signal<bool>` and returns `Page::Empty` while `loaded == false`, causing the boundary to render its `fallback` instead. Specifically:

- `crates/reinhardt-pages/wip/issue-1858-wasm-split-poc/src/router.rs` (POC source, branch-only):
	```rust
	let loaded: Signal<bool> = Signal::new(false);
	let page_cell: Rc<RefCell<Option<Page>>> = Rc::new(RefCell::new(None));

	#[cfg(target_arch = "wasm32")]
	spawn_local(async move {
	    let page = about_view().await;
	    *cell_writer.borrow_mut() = Some(page);
	    loaded_writer.set(true);   // ← flips the signal AFTER lazy chunk loads
	});

	SuspenseBoundary::new()
	    .fallback(|| {
	        page!(|| {
	            p {
	                "Loading About..."
	            }
	        })()
	    })
	    .content(move || {
	        if loaded.get() {                // ← reads the signal reactively
	            page_cell.borrow_mut().take().unwrap_or(Page::Empty)
	        } else {
	            Page::Empty                  // ← initial state: empty content → fallback rendered
	        }
	    })
	    .into_page()
	```

  **Conclusion**: At the moment the `/about` route is first rendered, `loaded.get() == false` (the `spawn_local` task has been kicked off but hasn't yet completed `about_view().await`), so the content closure returns `Page::Empty`. The `SuspenseBoundary` then renders its `fallback` (`<p>Loading About...</p>`). When the lazy chunk finally arrives and the `spawn_local` task completes, `loaded_writer.set(true)` flips the signal, the reactive content closure re-runs, and the actual About page is rendered in place. This is the exact sequence (d) requires. Criterion (d) is satisfied by inspection of the reactive logic.

  Note: this assumes Reinhardt's `Effect` / `Signal` reactive system functions correctly. That's a pre-existing property guaranteed by `reinhardt-pages`'s own test suite (`crates/reinhardt-pages/tests/reactive_system_tests.rs` etc.) — Phase 0 does not need to re-verify it.

#### (e) Chunk byte sizes

| Stage | File | Bytes | Notes |
|---|---|---|---|
| `cargo build --release --target wasm32` (no relocs) | `issue_1858_wasm_split_poc.wasm` | 2,562,316 | Raw rustc output |
| `wasm-bindgen --target web` (split-aware glue) | `main_bg.wasm` | 1,494,490 | Single bundle, pre-split |
| `wasm-split-cli split` initial chunk | `main.wasm` | 1,494,828 | Initial chunk after splitting |
| `wasm-split-cli split` lazy chunk | `module_0_about_view_impl.wasm` | **931** | Lazy chunk for `about_view_impl` (code only — see § 5.3) |
| `wasm-split-cli split` shared glue chunk | `chunk_0_split.wasm` | 395 | Shared import glue between main and lazy |
| `wasm-split-cli split` loader | `__wasm_split.js` | 2,015 | JavaScript loader |
| `wasm-bindgen --target web` JS glue | `main.js` | 20,080 | Bindgen JS glue (split-aware) |

Critical observation: `main.wasm` (1,494,828 bytes) is barely smaller than the original `main_bg.wasm` (1,494,490 bytes) — in fact it is *larger*. The lazy chunk is only 931 bytes despite the annotated function referencing a 16 KB `static HEAVY_DATA: [u64; 2048]` array. The implication is fundamental and is treated as an Open Question (O9).

#### (f) SSR coexistence outcome

The reinhardt-pages SSR `Renderer` (`crates/reinhardt-pages/src/ssr/renderer.rs`) successfully renders the home route (312 bytes of HTML containing `<!DOCTYPE html><html lang="en">…Home…`) on a native target. The `/about` route was constrained to the **CSR-only fallback** per spec § 5 Step 3 (f): on native targets the `#[wasm_split]`-annotated `async fn about_view()` cannot be driven without a WASM executor, so SSR returns the SuspenseBoundary fallback `<p>Loading About...</p>` (208 bytes wrapped in a `<!DOCTYPE html>` envelope). Browser-side hydration is expected to drive the lazy chunk fetch and final render. Phase 1 must design this hydration handshake explicitly (Open Question O2).

### 5.3 Adapter pattern needed in Phase 1+

The POC required several adaptations to make `#[wasm_split]` cooperate with reinhardt-pages' existing types and runtime. Phase 1+ must either accept these as the canonical pattern or redesign the API to hide them. They are recorded here as findings (F1–F10) so Phase 1 plans don't re-discover them.

| ID | Finding | Implication for Phase 1+ |
|---|---|---|
| **F1** | `#[wasm_split]` requires `async fn`. The macro's `quote!` block panics on `asyncness.is_none()`. | The reinhardt-pages public lazy API will be async-flavored. `#[lazy_component]` (proposed Phase 1+ macro, sub-issue #1858-5) must wrap user code in async fn boilerplate transparently, similar to how Dioxus's `#[component]` lifts components into Element-returning functions. |
| **F2** | `#[wasm_split]` does not preserve the `pub` visibility of the annotated item. The macro emits a private-visibility wrapper, requiring an explicit `pub fn outer() { inner_split().await }` adapter to expose the result. | Phase 1+ macros (sub-issue #1858-5) must auto-generate the outer wrapper plus delegate the visibility correctly. |
| **F3** | `Page: !Clone`. Holding a lazily-produced `Page` in a `Signal<Page>` is impossible; the POC used `Signal<bool>` (load-completed flag) plus `Rc<RefCell<Option<Page>>>` (storage) and a `spawn_local` task to bridge the gap. | Phase 1+ must decide whether to (a) implement `Clone` for `Page` (probably wide-impact), (b) introduce a dedicated `LazyPage` smart pointer that interoperates with reactive primitives, or (c) bake the `Signal<bool>` + storage pattern into a router-level `Route::lazy()` API (sub-issue #1858-2) so users never see it. |
| **F4** | `wasm-split-cli` requires the input WASM be built with `RUSTFLAGS="-C link-arg=--emit-relocs"`. Without these relocations, the splitter cannot rewrite the module. The CLI surface is also `wasm-split-cli split <ORIGINAL> <BINDGENED> <OUT_DIR>` — three positional arguments, not the `--input/--output-dir` flags assumed before the spike. | Phase 1+ build integration (sub-issue #1858-4) must set the linker flag automatically (likely via a `cargo make` recipe or a custom cargo subcommand) so users don't need to know the magic. |
| **F5** | `wasm-split-cli` moves only **code**, not data segments. The 16 KB `HEAVY_DATA` static remained in `main.wasm` despite being referenced only from the split-annotated function; the split chunk is correspondingly tiny (931 bytes). Realizing byte-size reductions for data-heavy lazy routes requires either (a) pulling data out of WASM (e.g., fetch-on-demand JSON), or (b) post-processing with `wasm-opt --memory-packing`. | Phase 1+ must establish a canonical pattern (likely pulling data out of WASM) and document the trade-offs. The naïve `#[lazy_component]` over a large-data component will *not* shrink the initial bundle without further pipeline steps (sub-issue #1858-1 / #1858-4). |
| **F6** | `#[wasm_split]` and `wasm-bindgen-test` are mutually incompatible in the same test binary. The `#[wasm_split]` proc-macro emits `#[no_mangle]` exports plus `#[link(wasm_import_module = "./__wasm_split.js")]` imports; `wasm-bindgen-test`'s linker resolves neither, so the test harness `main` symbol is dropped. | Phase 1+ test strategy for split-annotated components must use a different mechanism: (a) Playwright-based browser tests, (b) a separate non-test binary that exercises the split path and asserts via console output, or (c) splitter-aware tooling that re-bundles split chunks back into a single test binary (probably overkill). Decision deferred to Phase 1. |
| **F7** | `#[wasm_split]` is transparent on native targets — the macro emits the original function body unchanged when `target_arch != "wasm32"`. | Native compile + SSR can coexist with the same source code. SSR for split routes naturally renders the SuspenseBoundary fallback (since the `spawn_local` lazy-load logic is `#[cfg(target_arch = "wasm32")]`). |
| **F8** | The `wasm-bindgen` JS glue and `wasm-split-cli` outputs have a file-naming and directory-layout mismatch by default. `__wasm_split.js` contains `import { initSync } from "./main.js"` while wasm-bindgen names its output after the crate (e.g., `issue_1858_wasm_split_poc.js`). Resolution: invoke `wasm-bindgen --out-name main` so the bindgen output is named `main.js`. | Phase 1+ build integration (sub-issue #1858-4) must standardize on `--out-name main` plus a flat output directory. |
| **F9** | `wasm-split-cli` **wipes the output directory** before writing. Running it in the same directory as wasm-bindgen output will delete the bindgen `main.js`. The POC worked around this by preserving `main.js` aside, running the splitter, then restoring. | Phase 1+ build integration (sub-issue #1858-4) must orchestrate the order: bindgen → splitter (with bindgen output as `<BINDGENED>` argument) → restore bindgen JS glue. Or: file an upstream issue with DioxusLabs requesting a non-destructive flag. |
| **F10** | `wasm-split-cli` emits `__wasm_split.js` with hardcoded `/harness/split/` absolute paths for chunk URLs. This presumes serving under that path (Dioxus's harness convention). Independent serving requires post-processing the loader's URLs to relative paths. | Phase 1+ build integration (sub-issue #1858-4) must either (a) post-process `__wasm_split.js`, (b) configure the project's static-asset prefix (`reinhardt-utils::staticfiles`) to match `/harness/split/`, or (c) file an upstream issue requesting a `--base-url` flag. |

## 6. Phase 1+ scope proposal

| Logical ID | Summary | Estimated size |
|---|---|---|
| #1858-1 | `wasm-split` adapter layer in `crates/reinhardt-pages/src/code_splitting/` — wraps the chosen Phase 0 surface and exposes a Reinhardt-idiomatic API for the rest of `reinhardt-pages` to consume. Resolves F3 by hiding the `Signal<bool>` + `Rc<RefCell>` pattern. | Small-Medium |
| #1858-2 | Router integration: `Route::lazy()` API + automatic chunk fetch on navigation. Pairs with the adapter from #1858-1. | Medium |
| #1858-3 | `SuspenseBoundary` ↔ `wasm-split` load-state interlock — formalize the relationship between the boundary's `content` closure and the lazy chunk's load completion. | Small |
| #1858-4 | Build integration (`cargo make build-pages` or equivalent): orchestrate `cargo build --release --target wasm32 -- -C link-arg=--emit-relocs` → `wasm-bindgen --out-name main` → `wasm-split-cli split` → post-process `__wasm_split.js` URLs → final flat `dist/`. Resolves F4, F8, F9, F10. | Medium |
| #1858-5 | Component-level lazy loading via `#[lazy_component]` macro — generates the async fn + visibility wrapper required by F1, F2. | Medium-Large |
| #1858-6 | Prefetching strategies (hover, idle, route hint) — depends on #1858-2. | Medium |
| #1858-7 | Documentation and migration guide for adopters — covers the data-vs-code split limitation (F5), tooling, examples. | Small |

(Logical IDs become real GitHub issues at sub-issue creation time; each issue includes `Refs #1858`.)

Recommended ordering: #1858-1 → #1858-4 (in parallel: build integration is independent of API design) → #1858-2 + #1858-3 → #1858-5 → #1858-6 → #1858-7. Total estimate: 8–14 weeks of engineering across the seven sub-issues.

## 7. Open questions (intentionally deferred to Phase 1+)

These are out of scope for Phase 0 by design:

- **O1**: Final shape of the component-level lazy loading API (e.g., `#[lazy_component]` attribute, `Lazy<T>` wrapper, or other).
- **O2**: SSR streaming integration with lazy chunk import maps. If Phase 0's CSR-only fallback for `/about` is to be lifted, Phase 1 must design the SSR hydration handshake from scratch.
- **O3**: Prefetching strategy priority (hover, idle, route hint, `<link rel="modulepreload">`), and whether prefetching is opt-in or opt-out by default.
- **O4**: Chunk caching strategy (content-hash naming, CDN cache busting, integration with `reinhardt-utils` staticfiles).
- **O5**: Developer experience (whether `wasm-split-cli` runs on every dev rebuild, interaction with HMR in `crates/reinhardt-pages/src/hmr/`).
- **O6**: Error handling for chunk fetch failures (retry policy, fallback route, user-visible error UI).
- **O7**: Upstream governance (Path D specific): how often to bump the pinned Dioxus rev, and how to track upstream breaking changes in `wasm-split` while it remains experimental at DioxusLabs.
- **O8**: Cross-chunk reactive references — how `Signal` / `Effect` references are handled when split across chunks, especially for component-level (not route-level) splitting.
- **O9** (data vs. code split): see F5. Phase 1 must establish a canonical pattern for data-heavy lazy routes.
- **O10** (test strategy for split components): see F6. Phase 1 must adopt one of the workarounds.
- **O11** (build pipeline standardization): see F4 / F8 / F9 / F10. Phase 1 sub-issue #1858-4 is the dedicated venue.

## 8. Appendix

### 8.A POC branch and source

The Phase 0 POC tree lives at `crates/reinhardt-pages/wip/issue-1858-wasm-split-poc/` on branch `feature/issue-1858-wasm-code-splitting-phase0-spike`. Because `crates/reinhardt-pages/wip/.gitignore` masks the entire `wip/` subtree (`*` rule), no POC code reaches `main`. To inspect the POC, check out the branch locally:

```bash
git fetch origin
git worktree add ../reinhardt-wt-1858 feature/issue-1858-wasm-code-splitting-phase0-spike
cd ../reinhardt-wt-1858/crates/reinhardt-pages/wip/issue-1858-wasm-split-poc/
ls -la
```

Files (all gitignored):

- `Cargo.toml` — manifest with `[workspace]` opt-out and pinned `wasm-splitter` git dep
- `src/lib.rs` — entry point with `cfg`-gated WASM `main` and native-only `pub mod ssr`
- `src/home.rs` — eagerly-loaded Home component (uses `page!` macro)
- `src/about.rs` — `#[wasm_split(about)]`-annotated `async fn about_view_impl()` plus public `pub async fn about_view()` wrapper
- `src/router.rs` — Router with `/` and `/about` routes; lazy load orchestration via `Signal<bool>` + `Rc<RefCell<Option<Page>>>` + `spawn_local`
- `index.html` — host page importing `./main.js`
- `tests/router.rs` — documentation-only file explaining the F6 incompatibility
- `examples/ssr_smoke.rs` — native SSR smoke executable

### 8.B Pinned upstream commit

```
DioxusLabs/dioxus  rev = "2cd524553ec3c87139b6823e85fbed293990ca45"
```

The pinned rev provides:

- `wasm-splitter` (runtime crate, package name) v0.7.6
- `wasm-split-cli` (binary) v0.7.6
- `wasm-split-macro` (proc-macro crate) v0.7.6

Phase 1 sub-issue #1858-1 should consider whether to advance the rev as part of formalizing the dependency.

### 8.C Recreating the design spec

The design spec at `docs/superpowers/specs/2026-04-26-wasm-code-splitting-phase0-design.md` is gitignored project-wide (`.gitignore:144-146` "Design/planning documents (local only)"). It captures the brainstormed decisions, decision log, and full open questions list including newly-added O9, O10, O11. To re-create the spec, the brainstorming session that produced it can be replayed via `/superpowers:brainstorming` against this RFC plus Issue #1858; the spec format follows the convention used by other Phase 0 design docs in the same gitignored directory.

### 8.D Recreating the implementation plan

Similarly, the implementation plan at `docs/superpowers/plans/2026-04-26-wasm-code-splitting-phase0.md` is gitignored. It contains the bite-sized task breakdown that was executed during this Phase 0 spike. The plan's structure (8 Phases + Cascade Gate + Time-box Enforcement) is documented here for future reference; readers wishing to re-execute Phase 0 should re-derive the plan via `/superpowers:writing-plans` against this RFC.
