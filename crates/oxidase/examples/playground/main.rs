use dioxus::prelude::*;
use oxidase::{bind_js, reset_module_registry, use_watcher, WatcherGuard};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoxRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowMetrics {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StreamPayload {
    pub value: f64,
    pub elapsed_ms: f64,
    pub is_done: bool,
}

bind_js!("examples/playground/test_bridge.ts"::*);

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // 1. Command states
    let mut command_status = use_signal(|| "Idle".to_string());

    // 2. Query states
    let mut query_result = use_signal(|| "Not measured".to_string());
    let mut query_error = use_signal(|| "None".to_string());
    let mut self_healing_status = use_signal(|| "Idle".to_string());

    // 3. Window Resize Watcher states
    let mut watcher_enabled = use_signal(|| false);
    let mut watcher_metrics = use_signal(|| None::<WindowMetrics>);

    use_watcher(move || {
        watcher_enabled().then(|| {
            watch_window_resize(move |metrics| {
                watcher_metrics.set(Some(metrics));
            })
        })
    });

    // 4. rAF Coalescing vs Immediate Stream states
    let mut target_event_count = use_signal(|| 10000);

    // 4.1 Immediate Watcher states
    let mut immediate_count = use_signal(|| 0);
    let mut immediate_last_val = use_signal(|| 0.0);
    let mut immediate_elapsed_ms = use_signal(|| 0.0);
    let mut immediate_running = use_signal(|| false);
    let mut immediate_guard = use_signal(|| None::<WatcherGuard>);

    // 4.2 rAF Coalesced Watcher states
    let mut raf_count = use_signal(|| 0);
    let mut raf_last_val = use_signal(|| 0.0);
    let mut raf_elapsed_ms = use_signal(|| 0.0);
    let mut raf_running = use_signal(|| false);
    let mut raf_guard = use_signal(|| None::<WatcherGuard>);

    // 4.3 Computed throughput and status strings

    let immediate_rate_text = if immediate_elapsed_ms() > 0.0 {
        format!("{:.1} 회/s", (immediate_count() as f64) / (immediate_elapsed_ms() / 1000.0))
    } else {
        "- 회/s".to_string()
    };
    let immediate_time_text = if immediate_elapsed_ms() > 0.0 {
        format!("{:.2}s", immediate_elapsed_ms() / 1000.0)
    } else {
        "-".to_string()
    };

    let raf_rate_text = if raf_elapsed_ms() > 0.0 {
        format!("{:.1} frames/s", (raf_count() as f64) / (raf_elapsed_ms() / 1000.0))
    } else {
        "- frames/s".to_string()
    };
    let raf_time_text = if raf_elapsed_ms() > 0.0 {
        format!("{:.2}s", raf_elapsed_ms() / 1000.0)
    } else {
        "-".to_string()
    };
    let raf_reduction_text = if raf_count() > 0 {
        let pct = (1.0 - (raf_count() as f64 / target_event_count() as f64)) * 100.0;
        format!("⚡ IPC 부하 {:.1}% 감소", pct)
    } else {
        String::new()
    };

    rsx! {

        div { style: "min-height: 100vh; background-color: #f8fafc; color: #0f172a; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; padding: 2rem;",
            div { style: "max-width: 900px; margin: 0 auto; display: flex; flex-direction: column; gap: 2rem;",

                // Header
                header { style: "background: white; padding: 1.75rem 2rem; border-radius: 12px; border: 1px solid #e2e8f0; box-shadow: 0 1px 3px rgba(0,0,0,0.05); display: flex; align-items: center; gap: 1.5rem;",
                    OxidaseIcon {}
                    div {
                        h1 {
                            id: "page-title",
                            style: "margin: 0 0 0.25rem 0; font-size: 1.75rem; font-weight: 700; color: #1e1b4b;",
                            "oxidase Live Runtime Proof"
                        }
                        p { style: "margin: 0; font-size: 0.875rem; color: #64748b;",
                            "Standalone Layer 1 browser FFI & runtime engine: Command, Query, Watcher, Self-Healing, and rAF Coalescing."
                        }
                    }
                }

                // Section 1: Command (Sync Fire-and-Forget)
                section {
                    id: "section-command",
                    style: "background: white; padding: 1.5rem; border-radius: 10px; border: 1px solid #e2e8f0; display: flex; flex-direction: column; gap: 1rem;",
                    h2 { style: "margin: 0; font-size: 1.25rem; font-weight: 600; color: #1e293b;",
                        "1. Command (Synchronous Fire-and-Forget)"
                    }
                    div { style: "display: flex; gap: 1rem;",
                        button {
                            id: "btn-run-command",
                            style: "background: #2563eb; color: white; padding: 0.5rem 1rem; border-radius: 6px; border: none; font-weight: 500; cursor: pointer;",
                            onclick: move |_| {
                                focus_element("cmd-target-input");
                                command_status.set("Invoked focus_element synchronously!".to_string());
                            },
                            "Focus Target Input"
                        }
                        button {
                            id: "btn-fail-command",
                            style: "background: #dc2626; color: white; padding: 0.5rem 1rem; border-radius: 6px; border: none; font-weight: 500; cursor: pointer;",
                            onclick: move |_| {
                                fail_command("nonexistent-element-id");
                                command_status
                                    .set("Invoked fail_command (Error caught safely in console)".to_string());
                            },
                            "Failing Command (Safe Catch)"
                        }
                    }
                    input {
                        id: "cmd-target-input",
                        placeholder: "I am the target element with id='cmd-target-input'",
                        style: "padding: 0.5rem 0.75rem; border: 1px solid #cbd5e1; border-radius: 6px;",
                    }
                    div {
                        id: "command-status-display",
                        style: "font-size: 0.875rem; color: #475569;",
                        "Command Status: {command_status}"
                    }
                }

                // Section 2: Query (Type-Safe Async RPC)
                section {
                    id: "section-query",
                    style: "background: white; padding: 1.5rem; border-radius: 10px; border: 1px solid #e2e8f0; display: flex; flex-direction: column; gap: 1rem;",
                    h2 { style: "margin: 0; font-size: 1.25rem; font-weight: 600; color: #1e293b;",
                        "2. Query (Async RPC & Self-Healing)"
                    }
                    div {
                        id: "query-measurement-box",
                        style: "width: 240px; height: 80px; background: #e0e7ff; border: 2px dashed #6366f1; border-radius: 6px; display: flex; align-items: center; justify-content: center; font-size: 0.875rem; font-weight: 500; color: #4338ca;",
                        "Target Box (240x80)"
                    }
                    div { style: "display: flex; gap: 1rem;",
                        button {
                            id: "btn-run-query",
                            style: "background: #059669; color: white; padding: 0.5rem 1rem; border-radius: 6px; border: none; font-weight: 500; cursor: pointer;",
                            onclick: move |_| {
                                spawn(async move {
                                    match get_bounding_rect("query-measurement-box").await {
                                        Ok(rect) => {
                                            query_result
                                                .set(
                                                    format!(
                                                        "x={:.1}, y={:.1}, width={:.1}, height={:.1}",
                                                        rect.x,
                                                        rect.y,
                                                        rect.width,
                                                        rect.height,
                                                    ),
                                                );
                                            query_error.set("None".to_string());
                                        }
                                        Err(e) => query_error.set(format!("{e}")),
                                    }
                                });
                            },
                            "Measure Box Rect"
                        }
                        button {
                            id: "btn-reset-and-heal",
                            style: "background: #7c3aed; color: white; padding: 0.5rem 1rem; border-radius: 6px; border: none; font-weight: 500; cursor: pointer;",
                            onclick: move |_| {
                                spawn(async move {
                                    reset_module_registry();
                                    self_healing_status
                                        .set(
                                            "Registry reset! Running query to trigger self-healing..."
                                                .to_string(),
                                        );
                                    match get_bounding_rect("query-measurement-box").await {
                                        Ok(rect) => {
                                            self_healing_status
                                                .set(
                                                    format!("Self-healing success! Got width={:.1}", rect.width),
                                                )
                                        }
                                        Err(e) => self_healing_status.set(format!("Failed: {e}")),
                                    }
                                });
                            },
                            "Reset Registry & Self-Heal"
                        }
                    }
                    div {
                        id: "query-result-display",
                        style: "font-size: 0.875rem; color: #475569;",
                        "Query Result: {query_result}"
                    }
                    div {
                        id: "self-healing-status-display",
                        style: "font-size: 0.875rem; color: #7c3aed;",
                        "Self-Healing: {self_healing_status}"
                    }
                }

                // Section 3: Standard Window Resize Watcher
                section {
                    id: "section-watcher",
                    style: "background: white; padding: 1.5rem; border-radius: 10px; border: 1px solid #e2e8f0; display: flex; flex-direction: column; gap: 1rem;",
                    h2 { style: "margin: 0; font-size: 1.25rem; font-weight: 600; color: #1e293b;",
                        "3. Continuous Watcher (Resize)"
                    }
                    button {
                        id: "btn-toggle-watcher",
                        style: if watcher_enabled() { "background: #ef4444; color: white; padding: 0.5rem 1rem; border-radius: 6px; border: none; font-weight: 500; cursor: pointer;" } else { "background: #10b981; color: white; padding: 0.5rem 1rem; border-radius: 6px; border: none; font-weight: 500; cursor: pointer;" },
                        onclick: move |_| {
                            let curr = watcher_enabled();
                            watcher_enabled.set(!curr);
                        },
                        if watcher_enabled() {
                            "Stop Window Resize Watcher"
                        } else {
                            "Start Window Resize Watcher"
                        }
                    }
                    div {
                        id: "watcher-status-display",
                        style: "font-size: 0.875rem; color: #475569;",
                        "Watcher Active: {watcher_enabled}"
                    }
                    div {
                        id: "watcher-metrics-display",
                        style: "font-size: 0.875rem; color: #059669; font-weight: 600;",
                        if let Some(metrics) = watcher_metrics() {
                            "Live Metrics: width={metrics.width}, height={metrics.height}"
                        } else {
                            "Live Metrics: None"
                        }
                    }
                }

                // Section 4: Declarative Opt-In rAF Coalescing Live Proof (DEC-10)
                section {
                    id: "section-raf-proof",
                    style: "background: white; padding: 1.5rem; border-radius: 10px; border: 2px solid #6366f1; display: flex; flex-direction: column; gap: 1.25rem;",
                    div {
                        h2 { style: "margin: 0; font-size: 1.25rem; font-weight: 700; color: #4338ca;",
                            "4. Declarative Opt-In rAF Coalescing Throughput Test (DEC-10)"
                        }
                        p { style: "margin: 0.25rem 0 0 0; font-size: 0.875rem; color: #64748b;",
                            "지정한 횟수만큼 1초 동안 이벤트를 연속 발생시키고, Rust에서 초당 몇 번의 이벤트를 수신하는지 실시간 측정합니다."
                        }
                    }

                    // Config Bar
                    div { style: "background: #f1f5f9; padding: 1rem 1.25rem; border-radius: 8px; display: flex; align-items: center; gap: 0.75rem;",
                        label { style: "font-weight: 600; font-size: 0.875rem; color: #334155;",
                            "발생 시도 횟수 (1초):"
                        }
                        input {
                            id: "input-target-count",
                            r#type: "number",
                            min: "100",
                            step: "100",
                            value: "{target_event_count}",
                            style: "width: 120px; padding: 0.375rem 0.5rem; border-radius: 6px; border: 1px solid #cbd5e1; font-weight: 600; font-size: 0.95rem; text-align: center;",
                            oninput: move |evt| {
                                if let Ok(val) = evt.value().parse::<u32>() {
                                    target_event_count.set(val);
                                }
                            },
                        }
                        span { style: "font-size: 0.8125rem; color: #64748b;", "회" }
                    }

                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem;",

                        // Left: Immediate Card
                        div { style: "background: #f8fafc; padding: 1.25rem; border-radius: 8px; border: 1px solid #e2e8f0; display: flex; flex-direction: column; gap: 1rem;",
                            div {
                                h3 { style: "margin: 0; font-size: 1.05rem; font-weight: 700; color: #0f172a;",
                                    "A. Default #[watcher] (Immediate)"
                                }
                                p { style: "margin: 0.25rem 0 0 0; font-size: 0.8125rem; color: #64748b;",
                                    "1:1 즉시 IPC 디스패치. 스로틀링 없이 모든 이벤트를 그대로 전달합니다."
                                }
                            }

                            button {
                                id: "btn-run-immediate-stream",
                                disabled: "{immediate_running}",
                                style: if immediate_running() { "background: #94a3b8; color: white; padding: 0.625rem 1rem; border-radius: 6px; border: none; font-weight: 600; cursor: not-allowed; font-size: 0.875rem;" } else { "background: #0284c7; color: white; padding: 0.625rem 1rem; border-radius: 6px; border: none; font-weight: 600; cursor: pointer; font-size: 0.875rem;" },
                                onclick: move |_| {
                                    immediate_count.set(0);
                                    immediate_last_val.set(0.0);
                                    immediate_elapsed_ms.set(0.0);
                                    immediate_running.set(true);
                                    let target = target_event_count() as f64;
                                    let guard = watch_immediate_stream(
                                        target,
                                        1000.0,
                                        move |payload: StreamPayload| {
                                            immediate_count.set(immediate_count() + 1);
                                            immediate_last_val.set(payload.value);
                                            immediate_elapsed_ms.set(payload.elapsed_ms);
                                            if payload.is_done {
                                                immediate_running.set(false);
                                            }
                                        },
                                    );
                                    immediate_guard.set(Some(guard));
                                },
                                if immediate_running() {
                                    "⏳ 1초 스트림 실행 중..."
                                } else {
                                    "스트림 실행 (Immediate)"
                                }
                            }

                            div { style: "padding: 0.875rem; background: white; border-radius: 6px; border: 1px solid #cbd5e1; display: flex; flex-direction: column; gap: 0.5rem; font-size: 0.875rem;",
                                div { style: "display: flex; justify-content: space-between; align-items: center;",
                                    span { style: "color: #64748b;", "수신 이벤트:" }
                                    strong {
                                        id: "immediate-recv-count",
                                        style: "color: #0284c7; font-size: 1.125rem;",
                                        "{immediate_count} 회"
                                    }
                                }
                                div { style: "display: flex; justify-content: space-between; align-items: center;",
                                    span { style: "color: #64748b;", "초당 수신 횟수:" }
                                    strong {
                                        id: "immediate-throughput",
                                        style: "color: #0369a1; font-size: 1.125rem;",
                                        "{immediate_rate_text}"
                                    }
                                }
                                div { style: "display: flex; justify-content: space-between; font-size: 0.8125rem; color: #64748b;",
                                    span {
                                        "소요 시간: "
                                        strong { "{immediate_time_text}" }
                                    }
                                    span {
                                        "최종 도달값: "
                                        strong { "{immediate_last_val}" }
                                    }
                                }
                                div { style: "padding-top: 0.25rem; border-top: 1px dashed #e2e8f0; font-size: 0.8125rem; text-align: right;",
                                    span { id: "immediate-status-badge",
                                        if immediate_running() {
                                            "⏳ 측정 진행 중..."
                                        } else if immediate_count() > 0 {
                                            "✅ 1:1 전량 수신 (부하 높음)"
                                        } else {
                                            "대기 중"
                                        }
                                    }
                                }
                            }
                        }

                        // Right: rAF Coalesced Card
                        div { style: "background: #eef2ff; padding: 1.25rem; border-radius: 8px; border: 1px solid #c7d2fe; display: flex; flex-direction: column; gap: 1rem;",
                            div {
                                h3 { style: "margin: 0; font-size: 1.05rem; font-weight: 700; color: #3730a3;",
                                    "B. Opt-In #[watcher(raf)]"
                                }
                                p { style: "margin: 0.25rem 0 0 0; font-size: 0.8125rem; color: #4338ca;",
                                    "requestAnimationFrame 기반 디스플레이 프레임 레이트 자동 압축 병합."
                                }
                            }

                            button {
                                id: "btn-run-raf-stream",
                                disabled: "{raf_running}",
                                style: if raf_running() { "background: #94a3b8; color: white; padding: 0.625rem 1rem; border-radius: 6px; border: none; font-weight: 600; cursor: not-allowed; font-size: 0.875rem;" } else { "background: #4f46e5; color: white; padding: 0.625rem 1rem; border-radius: 6px; border: none; font-weight: 600; cursor: pointer; font-size: 0.875rem;" },
                                onclick: move |_| {
                                    raf_count.set(0);
                                    raf_last_val.set(0.0);
                                    raf_elapsed_ms.set(0.0);
                                    raf_running.set(true);
                                    let target = target_event_count() as f64;
                                    let guard = watch_raf_stream(
                                        target,
                                        1000.0,
                                        move |payload: StreamPayload| {
                                            raf_count.set(raf_count() + 1);
                                            raf_last_val.set(payload.value);
                                            raf_elapsed_ms.set(payload.elapsed_ms);
                                            if payload.is_done {
                                                raf_running.set(false);
                                            }
                                        },
                                    );
                                    raf_guard.set(Some(guard));
                                },
                                if raf_running() {
                                    "⏳ 1초 스트림 실행 중..."
                                } else {
                                    "스트림 실행 (rAF Coalesced)"
                                }
                            }

                            div { style: "padding: 0.875rem; background: white; border-radius: 6px; border: 1px solid #c7d2fe; display: flex; flex-direction: column; gap: 0.5rem; font-size: 0.875rem;",
                                div { style: "display: flex; justify-content: space-between; align-items: center;",
                                    span { style: "color: #4338ca;", "수신 이벤트:" }
                                    strong {
                                        id: "raf-recv-count",
                                        style: "color: #4f46e5; font-size: 1.125rem;",
                                        "{raf_count} 회"
                                    }
                                }
                                div { style: "display: flex; justify-content: space-between; align-items: center;",
                                    span { style: "color: #4338ca;",
                                        "초당 수신 횟수 (프레임 레이트):"
                                    }
                                    strong {
                                        id: "raf-throughput",
                                        style: "color: #3730a3; font-size: 1.125rem;",
                                        "{raf_rate_text}"
                                    }
                                }
                                div { style: "display: flex; justify-content: space-between; font-size: 0.8125rem; color: #4338ca;",
                                    span {
                                        "소요 시간: "
                                        strong { "{raf_time_text}" }
                                    }
                                    span {
                                        "최종 도달값: "
                                        strong { "{raf_last_val}" }
                                    }
                                }
                                div { style: "padding-top: 0.25rem; border-top: 1px dashed #c7d2fe; font-size: 0.8125rem; display: flex; justify-content: space-between; align-items: center;",
                                    span { style: "color: #059669; font-weight: 600;",
                                        "{raf_reduction_text}"
                                    }
                                    span { id: "raf-status-badge",
                                        if raf_running() {
                                            "⏳ 측정 진행 중..."
                                        } else if raf_count() > 0 {
                                            "✅ 프레임 단위 압축 완료"
                                        } else {
                                            "대기 중"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Reset button
                    div { style: "display: flex; justify-content: flex-end;",
                        button {
                            id: "btn-reset-counters",
                            style: "background: #f1f5f9; color: #475569; padding: 0.5rem 1rem; border-radius: 6px; border: 1px solid #cbd5e1; cursor: pointer; font-size: 0.875rem;",
                            onclick: move |_| {
                                immediate_count.set(0);
                                immediate_last_val.set(0.0);
                                immediate_elapsed_ms.set(0.0);
                                immediate_running.set(false);
                                immediate_guard.set(None);

                                raf_count.set(0);
                                raf_last_val.set(0.0);
                                raf_elapsed_ms.set(0.0);
                                raf_running.set(false);
                                raf_guard.set(None);
                            },
                            "카운터 초기화 (Reset)"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn OxidaseIcon() -> Element {
    let svg_content = include_str!("../../../../assets/icon.svg");
    rsx! {
        div {
            style: "width: 56px; height: 56px; flex-shrink: 0; display: flex; align-items: center; justify-content: center;",
            dangerous_inner_html: "{svg_content}",
        }
    }
}

