# dioxus-js-bindgen

> **TypeScript and JavaScript interop for Dioxus.**

`dioxus-js-bindgen` is a compile-time FFI binding generator and runtime engine for [Dioxus](https://dioxuslabs.com). It transforms standard TypeScript and JavaScript files into strongly-typed synchronous Rust commands, asynchronous RPC queries, and leak-free RAII reactive watchers.

AST parsing, type stripping, and bundling are executed entirely in-memory using a Rust-native [SWC](https://swc.rs) engine—**requiring zero external toolchains (no Node.js, npm, or Bun)** in either local development or CI/CD pipelines.

---

## Table of Contents

1. [Key Capabilities](#key-capabilities)
2. [Interaction Models](#interaction-models)
3. [Quick Start](#quick-start)
4. [3-Tier Classification & Annotation Rules](#3-tier-classification--annotation-rules)
5. [Macro Import Syntax & Identifier Matching](#macro-import-syntax--identifier-matching)
6. [Compilation Diagnostics & Unsupported Patterns](#compilation-diagnostics--unsupported-patterns)
7. [Runtime Architecture & Fault Tolerance](#runtime-architecture--fault-tolerance)
   - [Zero-Build In-Memory SWC Pipeline](#zero-build-in-memory-swc-pipeline)
   - [Hybrid Lazy Loader & Double-Epoch IPC Cache](#hybrid-lazy-loader--double-epoch-ipc-cache)
   - [Asymmetric Recovery & Bounded Self-Healing](#asymmetric-recovery--bounded-self-healing)
   - [Watcher Wire Protocol & Idempotent Cleanup](#watcher-wire-protocol--idempotent-cleanup)
   - [Reactive `use_watcher` Hook](#reactive-use_watcher-hook)
8. [Type System & Serialization Contract](#type-system--serialization-contract)
9. [Error Taxonomy & Diagnostic Handling](#error-taxonomy--diagnostic-handling)
10. [Headless Testing & Drop Safety](#headless-testing--drop-safety)
11. [License](#license)

---

## Key Capabilities

- 📝 **Native TypeScript Colocation**: Colocate `.ts` or `.js` files directly alongside your Rust components.
- ⚡ **Zero External Dependencies**: AST parsing and type stripping run entirely in Rust via SWC. No Node.js, Bun, or npm required.
- 🎯 **Three Interaction Models**:
  - **Commands**: Trigger DOM actions (like `focus()` or `scroll()`) synchronously without `async/await` boilerplate.
  - **Queries**: Read browser measurements and data directly into strongly-typed Rust structs.
  - **Watchers**: Listen to continuous browser events (like resizing or clicks) with automatic cleanup.
- 🔄 **Signal-Reactive Subscriptions (`use_watcher`)**: Subscriptions automatically rebind when Dioxus signals change and clean up completely when components unmount—eliminating browser listener leaks.
- 🔀 **Natural Rust & JS Naming**: Call JavaScript `camelCase` functions naturally using idiomatic Rust `snake_case`, with support for selective imports and renaming (`as`).
- 🩺 **Early Error Detection**: Disallows unsupported JavaScript patterns (like static imports or default exports) during `cargo build` with clear compiler errors.
- 🧪 **Safe Headless Testing**: Runs safely in desktop `cargo test` environments without browser panics.

---

## Interaction Models

Exported TypeScript declarations are mapped into three distinct interaction models in Rust:

| Model | TypeScript Signature | Classification Rule | Generated Rust Signature / Type | Behavior & Guarantees |
| :--- | :--- | :--- | :--- | :--- |
| **Command** | `function doWork(...args): void` | **Inferred** (`void` return) or `#[command]` | `pub fn do_work(...args)` | Synchronous, fire-and-forget DOM manipulation. Serializes parameters and dispatches immediately without `.await` or `spawn`. Isolated inside browser `try-catch`. |
| **Query** | `async function fetchRect(...args): Promise<T>` | **Inferred** (`Promise`/value return) or `#[query]` | `pub async fn fetch_rect(...args) -> Result<T, JsError>` | Asynchronous bidirectional RPC returning strongly-typed Serde data. Captures JavaScript exceptions and includes bounded 1-retry self-healing.<br>*(Note: `Promise<void>` generates a Query to await completion; use `#[command]` to discard the promise and dispatch fire-and-forget).* |
| **Watcher** | `/** #[watcher] */`<br>`function watchEvents(...args, emit): () => void` | **Strictly Explicit (`#[watcher]` required in doc comments or Rust macro)** | `pub fn watch_events(..., emit) -> WatcherGuard` | Continuous event streams (e.g. `ResizeObserver`, pointer events). Generates a free function returning a unified `WatcherGuard` lifecycle handle that automatically executes the returned cleanup closure on Rust `Drop` or `.stop()`. |

---

## Quick Start

### 1. Author your TypeScript module

Create a `.ts` file alongside your component (e.g., `src/browser/dom.ts`):

```typescript
/**
 * Command: Synchronous fire-and-forget DOM action (inferred from void return)
 */
export function focusElement(elementId: string): void {
    document.getElementById(elementId)?.focus();
}

/**
 * Query: Asynchronous measurement returning typed data (inferred from Promise return)
 */
export async function measureElement(
    elementId: string
): Promise<[number, number, number, number] | null> {
    const el = document.getElementById(elementId);
    if (!el) return null;
    const rect = el.getBoundingClientRect();
    return [rect.left, rect.top, rect.width, rect.height];
}

/**
 * Watcher: Continuous event subscription with cleanup
 * #[watcher] -- MANDATORY: Watchers are NEVER auto-inferred
 * (must be declared in JS/TS doc comments or in Rust bind_js!)
 */
export function watchResize(
    elementId: string,
    emit: (dimensions: { width: number; height: number }) => void
): () => void {
    const el = document.getElementById(elementId);
    if (!el) return () => {};

    const observer = new ResizeObserver(([entry]) => {
        emit({
            width: entry.contentRect.width,
            height: entry.contentRect.height,
        });
    });

    observer.observe(el);

    // Return cleanup closure: automatically called when Rust watcher drops
    return () => {
        observer.disconnect();
    };
}
```

> **Note on JavaScript**: While TypeScript is the recommended first-class authoring language, pure untyped `.js` files are also supported via fallback inference. See [Untyped JavaScript Fallback](#untyped-javascript-fallback) below.

### 2. Bind the module in Rust

In your Rust module or component file:

```rust
use dioxus_js_bindgen::bind_js;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Dimensions {
    pub width: f64,
    pub height: f64,
}

// Bind all exported functions from TypeScript:
bind_js!("src/browser/dom.ts"::*);
```

Or selectively import with renaming:

```rust
bind_js!("src/browser/dom.ts"::{
    focus_element,
    measure_element as get_element_rect,
    watch_resize, // Automatically inherits #[watcher] from the TS doc comment!
});
```

### 3. Use in Dioxus Components

```rust
use dioxus::prelude::*;
use dioxus_js_bindgen::use_watcher;

#[component]
pub fn ResizableBox() -> Element {
    let mut dimensions = use_signal(|| (0.0, 0.0));
    let mut element_id = use_signal(|| "box-1".to_string());

    // 1. Reactive Watcher Hook: Specialized use_effect wrapper for subscriptions.
    // Automatically rebinds when element_id() changes and cleans up on unmount.
    use_watcher(move || {
        let id = element_id(); // <-- Signal read: tracks element_id as a reactive dependency
        Some(watch_resize(&id, move |dim: Dimensions| {
            dimensions.set((dim.width, dim.height)); // <-- Signal write: streams data without re-attaching!
        }))
    });

    rsx! {
        div {
            id: "{element_id}",
            style: "resize: both; overflow: auto; border: 1px solid #aaa; padding: 16px;",
            p { "Live Dimensions: {dimensions().0:.1}px × {dimensions().1:.1}px" }

            button {
                onclick: move |_| {
                    // 2. Synchronous Command: Fire and forget (no async/await or spawn needed!)
                    focus_element(&element_id());

                    // 3. Asynchronous Query: Type-safe RPC with Result<T, JsError>
                    // Note: Dioxus event listeners are synchronous closures (FnMut),
                    // so awaiting an async Query inside an event handler requires spawn(async move { ... }).
                    spawn(async move {
                        match get_element_rect(&element_id()).await {
                            Ok(Some(rect)) => tracing::info!("Bounding Rect: {:?}", rect),
                            Ok(None) => tracing::warn!("Element not mounted"),
                            Err(err) => tracing::error!("Failed to measure: {err}"),
                        }
                    });
                },
                "Focus & Measure"
            }
        }
    }
}
```

#### Command vs. Query: Choosing the Right Interaction Model

| Feature | Command (`pub fn`) | Query (`pub async fn`) |
| :--- | :--- | :--- |
| **Execution** | Synchronous, fire-and-forget | Asynchronous bidirectional RPC |
| **Return Value** | `()` (no return value) | `Result<T, JsError>` (strongly-typed deserialized data) |
| **Boilerplate** | Zero (`.await` or `spawn` not needed) | Requires `.await` (and `spawn` inside UI event handlers) |
| **Error Handling** | Isolated inside browser `try-catch` | Explicit Rust `Result` handling with bounded 1-retry self-healing |
| **Best Used For** | DOM mutations (`focus()`, `scroll()`, class changes) | Measurements (`getBoundingClientRect()`), data retrieval |

- **Commands** offer maximum developer ergonomics for fire-and-forget UI operations without async boilerplate.
- **Queries** provide type-safe asynchronous data fetching with structured error handling and bounded self-healing.
- **Why `spawn` is used for Queries in event handlers**: In Dioxus, UI event listeners (`onclick`, `oninput`, etc.) accept synchronous closures (`FnMut`). Because you cannot directly `.await` inside a synchronous closure, any asynchronous RPC call (such as a Query) triggered by a user click must be scheduled inside a background task via `spawn(async move { ... })`. Outside event handlers (such as inside `use_resource(move || async move { ... })`), queries can be awaited directly without `spawn`.

#### How `use_watcher` Works
- **Signal Read Dependencies vs. Callback Writes**:
  - Only signals **read** inside the factory closure (`element_id()`) are registered as reactive dependencies that rebind the watcher.
  - Signals **written** inside the event callback closure (`dimensions.set(...)` or `scroll_y.set(...)`) do **not** re-trigger `use_watcher`.
  - **High-Performance Continuous Streaming**: Continuous browser events (such as scrolling, mouse movements, or resizing) do *not* tear down or recreate the listener on every frame. The browser listener is attached once and simply invokes the Rust callback closure with incoming stream payloads.
- **Declarative On/Off with `Option<WatcherGuard>`**:
  - Return `Some(watch_*(...))` to activate the watcher subscription.
  - Return `None` to deactivate or pause the subscription (for example, when a dialog or popover is closed: `if !is_open() { return None; }`). If an existing watcher was active, returning `None` immediately drops it and executes browser cleanup.

  ```rust
  #[component]
  pub fn CollapsibleInspector() -> Element {
      let mut is_open = use_signal(|| false);
      let mut content_height = use_signal(|| 0.0);

      // 💡 Declarative On/Off: The watcher only runs when the panel is open.
      // Toggling `is_open` to false returns None, automatically dropping the active
      // WatcherGuard and executing the browser teardown cleanup closure immediately.
      use_watcher(move || {
          if !is_open() {
              return None; // Inactive: tears down running observer and stops stream
          }

          // Active: attaches new browser observer and starts streaming events
          Some(watch_resize("inspector-content", move |dim: Dimensions| {
              content_height.set(dim.height);
          }))
      });

      rsx! {
          button {
              onclick: move |_| is_open.toggle(),
              if is_open() { "Close Inspector" } else { "Open Inspector" }
          }
          if is_open() {
              div {
                  id: "inspector-content",
                  style: "border: 1px solid #ccc; padding: 12px; margin-top: 8px;",
                  p { "Live observed height: {content_height}px" }
              }
          }
      }
  }
  ```
- **Zero-Leak Drop Lifecycle**:
  - Whenever a tracked dependency signal changes (e.g. `element_id` changes from `"box-1"` to `"box-2"`), the previously active `WatcherGuard` is automatically dropped—which dispatches the browser cleanup closure—before the new watcher is initiated on the new element.
  - When the component unmounts from the DOM, the watcher guard drops and cleanly terminates the browser observer, preventing memory leaks and orphaned event listeners.
- **Dependency Isolation & `.peek()` Best Practice**:
  - Because `use_watcher` wraps `use_effect`, **any signal read (`signal()` or `.read()`) inside the `factory` closure is registered as a reactive dependency.**
  - **Rule of Thumb**: Only read signals that dictate the watcher's lifecycle boundary (such as the target element ID `element_id()` or the activation gate `is_open()`).
  - **Avoid Unrelated UI State**: Reading unrelated state (such as input text, hover counters, or search queries) inside the `factory` closure causes the browser watcher to disconnect, run teardown cleanup, and re-attach on every single keystroke!
  - **Non-Reactive Reads via `.peek()` (Dioxus Alternative to `untrack`)**: Unlike frameworks that provide a closure-wide `untrack(|| { ... })` function (e.g. Leptos, Solid), Dioxus utilizes **`.peek()`** at the individual signal level. Calling `signal.peek()` reads the underlying value *without* registering a subscription on the current reactive context.

  ```rust
  // ❌ Anti-pattern: Unintended Re-subscriptions on Unrelated State
  let mut search_query = use_signal(String::new);
  let is_tracking = use_signal(|| true);

  use_watcher(move || {
      // ⚠️ DANGER: Reading `search_query()` registers it as a reactive dependency!
      // Every single keystroke in the input box will tear down the active watcher
      // and re-create a brand-new browser observer unnecessarily.
      let _query = search_query();

      if !is_tracking() {
          return None;
      }

      Some(watch_resize("target-box", move |size| {
          // ...
      }))
  });

  // ✅ Best Practice: Read Only Lifecycle Triggers; Isolate Other State with `.peek()`
  use_watcher(move || {
      // 1. Reactive Lifecycle Gate: Only read signals that MUST trigger re-binding
      if !is_tracking() {
          // Returning None cleanly drops the active WatcherGuard and cleans up the browser observer
          return None;
      }

      // 2. Non-Reactive Inspection: Use `.peek()` to inspect state without subscribing
      // (Using .peek() prevents keystrokes or unrelated state from tearing down the watcher)
      let initial_query = search_query.peek();
      println!("Mounting watcher with initial query: {}", *initial_query);

      // 3. Setup Browser Watcher: Callback writes will NOT re-trigger `use_watcher`
      Some(watch_resize("target-box", move |size| {
          // Event callback writes (e.g. signal.set(...)) stream data safely without restarting the watcher
          tracing::info!("Target box resized: {:?}", size);
      }))
  });
  ```

---

## 3-Tier Classification & Annotation Rules

The procedural macro classifies each exported function using a strict **3-Tier Precedence Hierarchy**:

```
Tier 1: Macro Invocation Attributes (#[command], #[query], #[watcher])
   │ (Overrides everything)
   ▼
Tier 2: TypeScript Doc Comment Attributes (/** #[command] */, etc.)
   │ (Overrides AST inference)
   ▼
Tier 3: AST Return Type Inference (void -> Command, Promise/val -> Query)
   │
   └─► Note: Watcher is NEVER inferred at Tier 3!
```

### Classification Precedence Table

| Precedence | Source | Example | Rules |
| :--- | :--- | :--- | :--- |
| **Tier 1 (Highest)** | Rust `bind_js!` Macro Item | `#[watcher] watch_fn`<br>`#[query] custom_rpc` | Takes absolute precedence over TypeScript comments and AST inference. |
| **Tier 2** | TypeScript JSDoc / Comment | `/** #[watcher] */`<br>`// #[command]` | Declared directly above the exported function in `.ts`/`.js`. |
| **Tier 3 (Lowest)** | AST Return Type Inference | `function foo(): void`<br>`async function bar(): Promise<T>` | - `void` or no return $\rightarrow$ **Command**<br>- `Promise<T>` or concrete return value $\rightarrow$ **Query**<br>- **Watcher is NEVER inferred.** |

### Invariant Rules
1. **Higher-Order Functions**: If an exported function returns a function (`() => void`) but lacks an explicit `#[watcher]` attribute (at Tier 1 or Tier 2), compilation halts immediately with an error:
   ```text
   error: Higher-order functions returning functions are not supported in bind_js!. 
          Return concrete serializable data or use a Watcher callback (annotated with #[watcher]).
   ```
2. **Callbacks**: A Watcher must take an `emit: (payload: T) => void` callback and return a cleanup closure `() => void`.

### Concrete Classification Examples

#### 1. TypeScript Declarations (`browser/tools.ts`)
```typescript
// Tier 3 (Auto-inferred Command): void return -> synchronous Command
export function scrollWindow(x: number, y: number): void {
    window.scrollTo(x, y);
}

// Tier 3 (Auto-inferred Query): Promise return -> asynchronous Query
export async function fetchDocumentTitle(): Promise<string> {
    return document.title;
}

// Tier 2 (Doc Comment Override): Overrides Promise<void> to generate a synchronous Command
// Note: Return value (the Promise) is completely discarded (fire-and-forget dispatch)
/** #[command] */
export async function trackAnalytics(event: string): Promise<void> {
    await fetch("/api/log", { method: "POST", body: event });
}

// Tier 2 (Doc Comment Override): Promotes synchronous void to an asynchronous Query
// Useful when Rust needs to await DOM readiness or capture browser JS exceptions
/** #[query] */
export function validateElementMounted(id: string): void {
    if (!document.getElementById(id)) {
        throw new Error(`Element #${id} not found in DOM`);
    }
}

// Tier 2 (Doc Comment Watcher): Explicitly designated continuous event stream
/** #[watcher] */
export function watchWindowScroll(emit: (scrollY: number) => void): () => void {
    const handler = () => emit(window.scrollY);
    window.addEventListener("scroll", handler);
    return () => window.removeEventListener("scroll", handler);
}

// Unannotated function returning a closure:
// ⚠️ WARNING: If targeted in bind_js! without #[watcher], compilation FAILS with a higher-order function error!
export function customResizeListener(el: HTMLElement, emit: (w: number) => void): () => void {
    const ro = new ResizeObserver(([e]) => emit(e.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
}
```

#### 2. Rust Binding with Tier 1 Overrides (`src/lib.rs`)
```rust
bind_js!("browser/tools.ts"::{
    scroll_window,            // Tier 3: Inferred as Command -> pub fn scroll_window(x: f64, y: f64)
    fetch_document_title,     // Tier 3: Inferred as Query -> pub async fn fetch_document_title() -> Result<String, JsError>
    track_analytics,          // Tier 2: Doc comment #[command] honored -> pub fn track_analytics(event: &str) (Promise discarded)
    validate_element_mounted, // Tier 2: Doc comment #[query] honored -> pub async fn validate_element_mounted(id: &str) -> Result<(), JsError>
    watch_window_scroll,      // Tier 2: Doc comment #[watcher] honored -> pub fn watch_window_scroll(...) -> WatcherGuard

    // Tier 1 Macro Override: Takes absolute precedence over everything!
    // Binds the unannotated third-party function as a Watcher (compilation would fail without #[watcher]):
    #[watcher] custom_resize_listener as watch_custom_resize,
});
```

---

## Macro Import Syntax & Identifier Matching

The `bind_js!` macro supports flexible path resolution and identifier mapping:

### 1. Path Resolution
Paths are specified relative to `CARGO_MANIFEST_DIR` of the invoking crate:
```rust
bind_js!("src/foundation/browser/dom.ts"::*);
```

### 2. Wildcard vs. Selective Imports
- **Wildcard (`*`)**: Generates Rust bindings for every exported function in the file.
- **Selective Import**: Generates bindings only for specified exports:
  ```rust
  bind_js!("src/browser/dom.ts"::{
      focus_element,
      measure_element,
  });
  ```

### 3. Renaming (`as`)
Rename exported JavaScript functions into custom Rust identifiers:
```rust
bind_js!("src/browser/dom.ts"::{
    measure_element as get_bounding_box,
});
```

### 4. Automatic Casing Normalization

JavaScript conventions favor `camelCase`, while Rust conventions require `snake_case`. The macro automatically reconciles casing across all binding and renaming scenarios within a single import block:

```typescript
// TypeScript (dom.ts)
export function focusElement(elementId: string): void;
export function hideElement(elementId: string): void;
export async function measureElement(elementId: string): Promise<Rect>;
export async function calculateOffset(elementId: string): Promise<number>;
```

```rust
bind_js!("dom.ts"::{
    // 1. Direct Import with Rust snake_case:
    // Matches TS 'focusElement' -> generates idiomatic 'pub fn focus_element'
    focus_element,

    // 2. Direct Import with TS camelCase (auto-normalized to snake_case):
    // Matches TS 'hideElement' -> generates idiomatic 'pub fn hide_element'
    hideElement,

    // 3. Renaming with snake_case Alias (as):
    // Matches TS 'measureElement' -> generates custom 'pub async fn get_element_rect'
    measure_element as get_element_rect,

    // 4. Renaming with camelCase Alias (auto-normalized to snake_case):
    // Matches TS 'calculateOffset' -> generates normalized 'pub async fn get_calculated_offset'
    calculateOffset as getCalculatedOffset,
});
```

### 5. Private Declarations & Module State
Non-exported functions, internal classes, top-level constants, and module-scoped variables inside the `.ts` file are bundled transparently into the inlined JavaScript module. You can use private helpers freely:
```typescript
// Private helper (not exported, not bound to Rust)
function computeOffset(el: HTMLElement): number {
    return el.scrollTop + 10;
}

// Exported Command (bound to Rust)
export function scrollAdjusted(id: string): void {
    const el = document.getElementById(id);
    if (el) el.scrollTop = computeOffset(el);
}
```

---

## Compilation Diagnostics & Unsupported Patterns

To guarantee predictable runtime behavior and avoid hidden build-pipeline dependencies, `dioxus-js-bindgen` rejects unsupported JavaScript constructs at compile time with actionable diagnostics:

### 1. Top-Level Static Imports
- ❌ **Disallowed**:
  ```typescript
  import { computePosition } from "@floating-ui/dom"; // Compile Error!
  ```
- 🛑 **Diagnostic**:
  ```text
  error: Top-level static import is not supported in v1 bindable files. 
         Use preloaded globals (window.*) or dynamic import() inside an async query.
  ```
- ✅ **Recommended Alternatives**:
  - **Global Preload**: Load external scripts via CDN or `<script>` tags in `index.html`, and access them via `window.FloatingUIDOM`.
  - **Dynamic Import**: Use dynamic `await import(...)` inside an asynchronous Query:
    ```typescript
    export async function positionDropdown(anchorId: string, menuId: string): Promise<void> {
        const { computePosition } = await import("https://esm.sh/@floating-ui/dom");
        // ...
    }
    ```

### 2. Unannotated Higher-Order Functions
- ❌ **Disallowed**: Returning functions without `#[watcher]` annotation on binding targets.
- 🛑 **Diagnostic**:
  ```text
  error: Higher-order functions returning functions are not supported in bind_js!. 
         Return concrete serializable data or use a Watcher callback (annotated with #[watcher]).
  ```
- 🎯 **Target Scope & Non-Target Behavior**:
  - **Private / Internal Functions**: Non-exported helper functions returning functions inside the `.ts`/`.js` file (e.g. curried functions or event factory closures) are **never checked**. They bundle transparently into the module's private JavaScript scope.
  - **Wildcard Mode (`*`)**: When using `bind_js!("file.ts"::*)`, all exported functions in the module become binding targets and are strictly validated.
  - **Selective Mode (`::{ ... }`)**: When selectively binding specific functions, **only functions explicitly requested in the import list are validated**. If the file exports unrelated higher-order functions that are not in your Rust import list, they are safely ignored and do NOT trigger a compilation error.
- ✅ **Fix**: If the target function is an event subscription, annotate it with `/** #[watcher] */`. If it is a utility, return concrete serializable data.

### 3. Default Exports
- ❌ **Disallowed**:
  ```typescript
  export default function main() { ... } // Compile Error!
  ```
- 🛑 **Diagnostic**:
  ```text
  error: Default exports are not supported in bind_js!. 
         Use named exports ('export function name()') to avoid Rust identifier conflicts.
  ```
- ✅ **Fix**: Always use named exports (`export function myFunction() { ... }`).

---

## Runtime Architecture & Fault Tolerance

```
┌────────────────────────────────────────────────────────┐
│              Compile-Time (Proc Macro)                 │
│  "dom.ts" ──► [swc_core AST] ──► [Type Stripper]       │
│                     │                     │            │
│         [Classification & Diagnostics]    │ Inlined JS │
│                     │                     │            │
│  Rust AST ◄── [Rust Codegen Engine] ◄───┘            │
└─────────────────────────┬──────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────┐
│               Runtime Execution Model                  │
│                                                        │
│  Rust Side:                                            │
│   ├── static MODULE_LOADED_EPOCH: AtomicU64            │
│   ├── Command: pub fn(...) -> ()                       │
│   ├── Query: pub async fn(...) -> Result<T, JsError>   │
│   ├── Watcher: pub fn watch_<name>(...) -> WatcherGuard│
│   └── use_watcher(FnMut() -> Option<WatcherGuard>)     │
│                                                        │
│  IPC Boundary (dioxus::document::eval)                 │
│                                                        │
│  Browser Side:                                         │
│   ├── window.__DIOXUS_BINDGEN_MODULES__["{HASH}"]      │
│   └── window.__DIOXUS_WATCHERS: Map<sub_id, cleanup>   │
└────────────────────────────────────────────────────────┘
```

### Zero-Build In-Memory SWC Pipeline
During compilation, `bind_js!` uses `swc_core` to:
1. Parse the TypeScript AST into memory.
2. Validate signatures and enforce diagnostic invariants.
3. Strip type annotations, interfaces, and type aliases.
4. Hash the source file content deterministically.
5. Invert the module into an isolated IIFE that registers its exports into `window.__DIOXUS_BINDGEN_MODULES__["{HASH}"]`.

### Hybrid Lazy Loader & Double-Epoch IPC Cache
To avoid re-evaluating JavaScript code on every function call:
- **Rust Load Epoch**: Each generated module contains a `static MODULE_LOADED_EPOCH: AtomicU64 = AtomicU64::new(0)`.
- **Fast Path (~0ns)**: On invocation, Rust compares `MODULE_LOADED_EPOCH` with `dioxus_js_bindgen::internal::current_epoch()`. If they match, the module is known to be loaded in the browser, and dispatch proceeds immediately without initialization overhead.
- **Slow Path**: If epochs differ (first run or after a global reset), Rust evaluates the module bundle in the browser and updates the atomic epoch.
- **Global Cache Invalidation**: Calling `dioxus_js_bindgen::clear_js_cache()` (or legacy alias `reset_module_registry()`) increments the global epoch and clears the browser module cache. Note that this invalidates the module bundle evaluation cache so modules re-evaluate on next invocation; it does *not* automatically terminate active watchers or reset individual subscription lifecycle state (which remains owned by `WatcherGuard`).

### Asymmetric Recovery & Bounded Self-Healing
If the browser context loses module state (e.g. following full-page navigation or hot reload):
- **Query Self-Healing**: If a query receives a `MODULE_NOT_FOUND` signal from the browser, it resets `MODULE_LOADED_EPOCH`, re-injects the module bundle, and retries the query **at most once**. If it fails a second time, it returns `JsError::ModuleUnavailable`.
- **Command Best-Effort**: Commands log a `console.warn` in the browser and return immediately without blocking the Rust thread or crashing the UI.
- **Watcher Resilience**: Watchers reset the local load epoch upon encountering an unloaded module to ensure subsequent rebind attempts succeed.

### Watcher Wire Protocol & Idempotent Cleanup
1. **Subscription Registration**: Calling `watch_x(..., emit)` generates a globally unique 64-bit `subscription_id` and returns a `WatcherGuard`.
2. **Browser Storage**: The JavaScript watcher factory runs and places its cleanup closure into `window.__DIOXUS_WATCHERS.set(sub_id, cleanup)`.
3. **Continuous Streaming**: The browser watcher invokes `emit(payload)` whenever events occur, transmitting serialized JSON to Dioxus.
4. **Deterministic Teardown**: When the Rust watcher handle is dropped (or `.stop()` is called):
   - The background Dioxus task is cancelled.
   - A synchronous teardown eval is dispatched:
     ```javascript
     const cleanup = window.__DIOXUS_WATCHERS?.get(sub_id);
     if (cleanup) {
         try { cleanup(); } catch (e) { console.error(e); }
         window.__DIOXUS_WATCHERS.delete(sub_id);
     }
     ```
   - Teardown is 100% idempotent: subsequent calls or drops are safe no-ops.

### Reactive `use_watcher` Hook
Standard Dioxus hooks like `use_hook` require `Clone`, which directly conflicts with RAII cleanup structs. `dioxus-js-bindgen` provides `use_watcher`, a pure reactive hook wrapping `use_effect`:

```rust
pub fn use_watcher<W: 'static>(mut factory: impl FnMut() -> Option<W> + 'static) {
    let mut current_watcher = dioxus::prelude::use_signal(|| None::<W>);

    dioxus::prelude::use_effect(move || {
        let new_watcher = factory();
        current_watcher.set(new_watcher);
    });
}
```

- **Reactive Dependency Tracking**: Any Dioxus signals read inside `factory()` are registered as reactive dependencies.
- **Dynamic Rebinding**: When a signal changes (e.g. `is_open` toggles or `element_id` updates), `use_effect` re-runs:
  - Setting `new_watcher` automatically drops the previous watcher struct, immediately firing the browser cleanup.
  - If `factory()` returns `None`, no new watcher is created.
- **Unmount Safety**: When the host component unmounts, `current_watcher` drops, cleaning up all browser event listeners automatically.

---

## Type System & Serialization Contract

### 1. Primitive & Built-in Mappings

| TypeScript / JavaScript | Rust Parameter Type | Rust Return Type | Serialization |
| :--- | :--- | :--- | :--- |
| `void` / no return | N/A | `()` | Synchronous Command |
| `string` | `&str` | `String` | UTF-8 JSON string |
| `number` | `f64` | `f64` | Serde JSON number |
| `boolean` | `bool` | `bool` | Serde JSON boolean |
| `T[]` or `Array<T>` | `&[T]` | `Vec<T>` | JSON array |
| `T \| null \| undefined` | `Option<T>` | `Option<T>` | Nullable JSON value |
| `[A, B, ...]` | `(A, B, ...)` | `(A, B, ...)` | Fixed-size tuple |
| `Promise<T>` | N/A | `Result<T, JsError>` | Asynchronous Query RPC |
| `(event: T) => void` | `impl FnMut(T) + 'static` | `WatcherGuard` | Event subscription stream returning unified lifecycle guard |
| `any` / untyped JS | `serde_json::Value` | `serde_json::Value` | Arbitrary JSON AST |

### 2. User-Defined Structures & Discriminated Unions
The macro does not synthesize Rust struct or enum definitions from TypeScript interfaces. Instead, **the consumer authors the corresponding Rust types** in scope with `#[derive(Serialize, Deserialize)]`:

#### Object Interfaces
```typescript
export interface ElementRect {
    x: number;
    y: number;
    width: number;
    height: number;
}
export async function getRect(id: string): Promise<ElementRect> { ... }
```
```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ElementRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

bind_js!("src/dom.ts"::get_rect);
```

#### Discriminated Unions (Tagged Enums)
```typescript
export type DismissEvent =
    | { kind: "pointer_down"; path_ids: string[] }
    | { kind: "escape" };
```
```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DismissEvent {
    PointerDown { path_ids: Vec<String> },
    Escape,
}
```

---

## Error Taxonomy & Diagnostic Handling

All asynchronous queries return `Result<T, JsError>`. The `JsError` enum provides exhaustive categorization of browser and transport failures:

```rust
pub enum JsError {
    /// JavaScript runtime exception thrown inside the browser (message and stack trace)
    Exception {
        message: String,
        stack: Option<String>,
    },
    /// Low-level Dioxus eval transport communication failure
    Transport(String),
    /// Serde JSON payload deserialization failure
    Deserialization(String),
    /// Module failed to load even after bounded 1-retry self-healing
    ModuleUnavailable(String),
}
```

### Comprehensive Matching Example
```rust
match get_bounding_box("target-id").await {
    Ok(Some(rect)) => println!("Measured: {rect:?}"),
    Ok(None) => println!("Element not mounted in DOM"),
    Err(JsError::Exception { message, stack }) => {
        eprintln!("Browser JS Error: {message}\nStack: {stack:?}");
    }
    Err(JsError::Deserialization(err)) => {
        eprintln!("Schema mismatch between TS and Rust: {err}");
    }
    Err(JsError::Transport(err)) => {
        eprintln!("Dioxus IPC transport lost: {err}");
    }
    Err(JsError::ModuleUnavailable(name)) => {
        eprintln!("Module failed to register: {name}");
    }
}
```

## Headless Testing & Drop Safety

In non-browser environments (such as unit tests running via `cargo test` on desktop/server targets):
- **Runtime Presence Checks**: Functions check `dioxus::core::Runtime::try_current()` before attempting IPC dispatch.
- **Unwind Protection**: Watcher `Drop` and cleanup dispatch are wrapped in `std::panic::catch_unwind`, preventing teardown panics when dropping watcher handles outside of an active Dioxus thread.
- **Test Isolation**: `dioxus_js_bindgen::clear_js_cache()` (or legacy alias `reset_module_registry()`) can be safely invoked between tests to invalidate module evaluation caches cleanly.


---

## Untyped JavaScript Fallback

While `dioxus-js-bindgen` is architected as **TypeScript-first** for zero-cost compile-time type extraction, you can also bind pure untyped `.js` files. When binding JavaScript files without TypeScript types:

1. **Parameter Fallback**: Because plain JavaScript lacks static parameter annotations, parameters fall back to `serde_json::Value`:
   ```javascript
   // src/browser/tools.js
   export function logMessage(tag, payload) {
       console.log(`[${tag}]`, payload);
   }
   ```
   Generates:
   ```rust
   pub fn log_message(tag: serde_json::Value, payload: serde_json::Value);
   ```
2. **Return Types**: Functions returning unannotated dynamic values default to Query `Result<serde_json::Value, JsError>`.
3. **Watcher Annotation**: Watchers in `.js` must declare `#[watcher]` explicitly in JSDoc or via `bind_js!` macro invocation attributes:
   ```javascript
   /**
    * #[watcher]
    */
   export function watchWindowResize(emit) {
       const handler = () => emit({ width: window.innerWidth, height: window.innerHeight });
       window.addEventListener("resize", handler);
       return () => window.removeEventListener("resize", handler);
   }
   ```
4. **Best Practice**: Prefer TypeScript (`.ts`) whenever possible to obtain idiomatic, strongly-typed Rust signatures (`&str`, `f64`, `bool`, `&[T]`, `Vec<T>`).


---

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
