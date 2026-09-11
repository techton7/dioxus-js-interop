//! InteropRuntime - Core generic cross-platform runtime engine for Dioxus.

pub mod geometry;
pub use geometry::*;

use crate::bind_js;
use crate::JsError;

// Bind the TypeScript runtime using bind_js! macro
bind_js!("src/runtime/interop_runtime.ts"::{
    measure_rect as measure_rect_raw,
    get_viewport as get_viewport_raw,
});

/// Measures the bounding client rectangle of an element by ID.
pub async fn measure_rect(element_id: &str) -> Result<Option<Rect>, JsError> {
    measure_rect_raw(element_id).await
}

/// Queries current viewport dimensions and scroll offsets.
pub async fn get_viewport() -> Result<Viewport, JsError> {
    get_viewport_raw().await
}
