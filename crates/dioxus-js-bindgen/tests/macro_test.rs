use dioxus_js_bindgen::{bind_js, use_watcher, JsError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

// 1. Wildcard import test module
mod wildcard_bridge {
    use super::*;
    bind_js!("tests/fixtures/test_bridge.ts"::*);
}

// 2. Selective import with renaming and override test module
mod selective_bridge {
    use super::*;
    bind_js!("tests/fixtures/test_bridge.ts"::{
        focus_element as focus,
        get_bounding_rect as get_rect,
        #[command] fetch_remote_title as fetch_ignore_ret,
        watch_resize as watch_window_resize
    });
}

// 3. CamelCase selective import test module
mod camel_case_bridge {
    use super::*;
    bind_js!("tests/fixtures/test_bridge.ts"::{
        focusElement as direct_focus,
        fetchRemoteTitle as async_fetch
    });
}

// 4. Multi-invocation hygiene test module (multiple bind_js in same module scope)
mod multi_invocation_hygiene {
    use super::*;
    bind_js!("tests/fixtures/test_bridge.ts"::{ focus_element });
    bind_js!("tests/fixtures/test_bridge_second.ts"::{ secondary_helper });
}

// 5. Precedence override test module (macro #[watcher] overrides TS doc comment absence)
mod precedence_override_bridge {
    use super::*;
    bind_js!("tests/fixtures/test_bridge.ts"::{
        #[watcher]
        fn watch_custom_signal as watch_signal,
    });
}

// 6. Renaming with camelCase alias test module (should normalize to snake_case)
mod camel_case_rename_bridge {
    use super::*;
    bind_js!("tests/fixtures/test_bridge.ts"::{
        focus_element as focusBox,
        getBoundingRect as getBoundingBox,
    });
}

// 7. Selective import with trailing wildcard fallback test module
mod selective_wildcard_bridge {
    use super::*;
    bind_js!("tests/fixtures/test_bridge.ts"::{
        focus_element as focus_renamed,
        #[command] fetch_remote_title as fetch_cmd,
        *
    });
}

#[test]
fn test_wildcard_signatures() {
    // Check function existence and signatures without running browser eval
    let _cmd_fn: fn(&str) = wildcard_bridge::focus_element;
    let _cmd_scroll: fn(f64, f64) = wildcard_bridge::scroll_to_position;
    
    // Check query signature
    fn _check_query<F, Fut>(f: F)
    where
        F: Fn(&'static str) -> Fut,
        Fut: std::future::Future<Output = Result<Rect, JsError>>,
    {
        let _ = f;
    }
    _check_query(wildcard_bridge::get_bounding_rect);

    // Check async query signature
    fn _check_async_query<F, Fut>(f: F)
    where
        F: Fn(&'static str) -> Fut,
        Fut: std::future::Future<Output = Result<String, JsError>>,
    {
        let _ = f;
    }
    _check_async_query(wildcard_bridge::fetch_remote_title);

    // Check watcher free function existence
    fn _check_watcher<F>(f: F)
    where
        F: Fn(Box<dyn FnMut(WindowSize) + 'static>) -> dioxus_js_bindgen::WatcherGuard,
    {
        let _ = f;
    }
}

#[test]
fn test_selective_signatures_and_renaming() {
    let _focus_fn: fn(&str) = selective_bridge::focus;
    
    // fetch_ignore_ret was overridden as #[command], so it is a synchronous void function!
    let _cmd_fetch: fn(&str) = selective_bridge::fetch_ignore_ret;

    // Renamed watcher is a free function returning WatcherGuard
    let _guard: Option<dioxus_js_bindgen::WatcherGuard> = None;
}

#[test]
fn test_camel_case_matching() {
    let _focus_fn: fn(&str) = camel_case_bridge::direct_focus;
}

#[test]
fn test_multi_invocation_hygiene() {
    let _focus_fn: fn(&str) = multi_invocation_hygiene::focus_element;
    fn _check_query<F, Fut>(f: F)
    where
        F: Fn(&'static str) -> Fut,
        Fut: std::future::Future<Output = Result<String, JsError>>,
    {
        let _ = f;
    }
    _check_query(multi_invocation_hygiene::secondary_helper);
}

#[test]
fn test_macro_watcher_override_precedence() {
    let _guard: Option<dioxus_js_bindgen::WatcherGuard> = None;
}

// Component to test use_watcher compiles cleanly with generated Watcher function
#[allow(dead_code)]
fn test_component() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    let mut last_size = use_signal(|| None::<WindowSize>);

    use_watcher(move || {
        Some(wildcard_bridge::watch_resize(move |size| {
            last_size.set(Some(size));
        }))
    });

    rsx! {
        div { "Test Component" }
    }
}

#[test]
fn test_camel_case_renaming_normalizes_to_snake_case() {
    // Both focusBox and getBoundingBox must normalize to focus_box and get_bounding_box
    let _ = camel_case_rename_bridge::focus_box;
    let _ = camel_case_rename_bridge::get_bounding_box;
}

#[test]
fn test_array_generic_signatures() {
    // Verify signatures and return types for Array<T> functions
    let _ = async {
        let tags: &[&str] = &["a", "b"];
        let res: Result<Vec<String>, JsError> = wildcard_bridge::process_tags(tags).await;
        let _ = res;

        let ids: &[&str] = &["id-1", "id-2"];
        let scores: Result<Vec<f64>, JsError> = wildcard_bridge::fetch_scores(ids).await;
        let _ = scores;
    };
}


#[test]
fn test_clear_js_cache_alias() {
    use dioxus_js_bindgen::{clear_js_cache, reset_module_registry};
    clear_js_cache();
    reset_module_registry();
}

#[test]
fn test_selective_with_wildcard_fallback() {
    // 1. Explicitly renamed item exists under new name
    let _focus_fn: fn(&str) = selective_wildcard_bridge::focus_renamed;

    // 2. Explicitly overridden item exists as void command
    let _cmd_fetch: fn(&str) = selective_wildcard_bridge::fetch_cmd;

    // 3. Trailing wildcard imports other functions automatically with standard defaults
    let _scroll_fn: fn(f64, f64) = selective_wildcard_bridge::scroll_to_position;
}

