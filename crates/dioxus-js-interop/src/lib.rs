//! # dioxus-js-interop
//!
//! Unified JS/TS FFI and InteropRuntime Engine for Dioxus (Web, Desktop, Mobile).

extern crate self as dioxus_js_interop;

pub use dioxus_js_interop_macro::bind_js;
pub use serde;
pub use serde_json;
pub use tracing;

pub mod watcher_guard;
pub use watcher_guard::WatcherGuard;

pub mod runtime;

use std::sync::atomic::{AtomicU64, Ordering};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// JS FFI 에러 타입
#[derive(Debug, Error, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JsError {
    #[error("JavaScript Exception: {message}\nStack: {stack:?}")]
    Exception {
        message: String,
        stack: Option<String>,
    },
    #[error("Transport Error: {0}")]
    Transport(String),
    #[error("Module Unavailable: '{0}' (Context may have been reset)")]
    ModuleUnavailable(String),
    #[error("Deserialization Error: {0}")]
    Deserialization(String),
}

/// Query 비동기 RPC 응답 페이로드
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub stack: Option<String>,
}

impl<T> RpcResponse<T> {
    pub fn as_error(&self) -> Option<&str> {
        if !self.ok {
            self.error.as_deref()
        } else {
            None
        }
    }

    pub fn into_result(self) -> Result<T, JsError> {
        if self.ok {
            self.data.ok_or_else(|| JsError::Transport("Missing data in success response".into()))
        } else {
            Err(JsError::Exception {
                message: self.error.unwrap_or_else(|| "Unknown JS error".into()),
                stack: self.stack,
            })
        }
    }
}

/// 브라우저에 주입된 JS 모듈 캐시와 Rust 측 Epoch를 무효화하여 모든 바인딩 모듈이 다음 호출 시 브라우저에 재평가/재등록되도록 합니다.
///
/// **계약 보장**: 이 함수는 모듈 번들의 평가 캐시와 Epoch를 무효화할 뿐이며, 현재 실행 중인 활성 `WatcherGuard`나
/// 구독 이벤트 스트림의 생명주기를 전역으로 자동 중단하지는 않습니다. (개별 감시자 정리는 `WatcherGuard`의 Drop을 통해 수행됩니다.)
pub fn clear_js_cache() {
    internal::GLOBAL_EPOCH.fetch_add(1, Ordering::SeqCst);
    let _ = std::panic::catch_unwind(|| {
        let _ = dioxus::document::eval(
            r#"
            if (window.__DIOXUS_BINDGEN_MODULES__) {
                window.__DIOXUS_BINDGEN_MODULES__ = {};
            }
            "#,
        );
    });
}

/// 하위 호환성을 위한 레거시 별칭 (대신 [`clear_js_cache`] 사용 권장)
pub use clear_js_cache as reset_module_registry;

/// Dioxus Signal을 자동으로 추적하여 의존성 변경 시 이전 감시자를 Drop하고 새 감시자를 시작하는 순수 리액티브 훅
pub fn use_watcher<W: 'static>(mut factory: impl FnMut() -> Option<W> + 'static) {
    let mut current_watcher = dioxus::prelude::use_signal(|| None::<W>);

    dioxus::prelude::use_effect(move || {
        let new_watcher = factory();
        current_watcher.set(new_watcher);
    });
}

#[doc(hidden)]
pub mod internal {
    use super::*;

    pub static GLOBAL_EPOCH: AtomicU64 = AtomicU64::new(1);
    static NEXT_SUB_ID: AtomicU64 = AtomicU64::new(1);

    #[inline]
    pub fn current_epoch() -> u64 {
        GLOBAL_EPOCH.load(Ordering::Acquire)
    }

    #[inline]
    pub fn next_subscription_id() -> u64 {
        NEXT_SUB_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub fn dispatch_cleanup(sub_id: u64) {
        if dioxus::core::Runtime::try_current().is_none() {
            return;
        }
        let _ = std::panic::catch_unwind(|| {
            let _ = dioxus::document::eval(&format!(
                r#"
                (function() {{
                    const c = window.__DIOXUS_WATCHERS?.get({sub_id});
                    if (c) {{
                        try {{ c(); }} catch(e) {{ console.error("[Watcher Cleanup Error]:", e); }}
                        window.__DIOXUS_WATCHERS.delete({sub_id});
                    }}
                }})();
                "#
            ));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_response_success() {
        let resp = RpcResponse {
            ok: true,
            data: Some(42),
            error: None,
            stack: None,
        };
        assert_eq!(resp.into_result().unwrap(), 42);
    }

    #[test]
    fn test_rpc_response_error() {
        let resp: RpcResponse<i32> = RpcResponse {
            ok: false,
            data: None,
            error: Some("Element not found".into()),
            stack: Some("stack trace".into()),
        };
        match resp.into_result() {
            Err(JsError::Exception { message, stack }) => {
                assert_eq!(message, "Element not found");
                assert_eq!(stack, Some("stack trace".into()));
            }
            _ => panic!("Expected JsError::Exception"),
        }
    }

    #[test]
    fn test_epoch_increment() {
        let initial = internal::current_epoch();
        clear_js_cache();
        assert_eq!(internal::current_epoch(), initial + 1);

        // Verify backward-compatible alias
        reset_module_registry();
        assert_eq!(internal::current_epoch(), initial + 2);
    }

    #[test]
    fn test_subscription_id_increment() {
        let id1 = internal::next_subscription_id();
        let id2 = internal::next_subscription_id();
        assert!(id2 > id1);
    }

    #[test]
    fn test_js_error_display() {
        let err = JsError::ModuleUnavailable("module_123".into());
        assert!(err.to_string().contains("module_123"));

        let err2 = JsError::Transport("failed to connect".into());
        assert!(err2.to_string().contains("failed to connect"));
    }

    #[test]
    fn test_rpc_as_error() {
        let resp: RpcResponse<()> = RpcResponse {
            ok: false,
            data: None,
            error: Some("MODULE_NOT_FOUND".into()),
            stack: None,
        };
        assert_eq!(resp.as_error(), Some("MODULE_NOT_FOUND"));

        let ok_resp: RpcResponse<i32> = RpcResponse {
            ok: true,
            data: Some(10),
            error: None,
            stack: None,
        };
        assert_eq!(ok_resp.as_error(), None);
    }

    #[test]
    fn test_watcher_guard_lifecycle() {
        let guard = WatcherGuard::new("watch_resize", 42, None);
        assert_eq!(guard.name(), "watch_resize");
        assert_eq!(guard.subscription_id(), 42);
        let debug_str = format!("{:?}", guard);
        assert!(debug_str.contains("watch_resize"));
        assert!(debug_str.contains("42"));
        // Drop test outside runtime - must not panic
        drop(guard);
    }
}
