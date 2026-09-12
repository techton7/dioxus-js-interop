(function() {
    const mod = window.__DIOXUS_BINDGEN_MODULES__?.["__MODULE_HASH__"];
    if (!mod) {
        console.error("[dioxus-js-interop]: Module '__MODULE_HASH__' not found. Cannot start watcher.");
        dioxus.send({ __bindgen_err: "MODULE_NOT_FOUND" });
        return;
    }
    if (!window.__DIOXUS_WATCHERS) window.__DIOXUS_WATCHERS = new Map();
    let __raf_pending = null;
    let __raf_id = null;
    const emit = (val) => {
        __raf_pending = val;
        if (__raf_id === null) {
            __raf_id = requestAnimationFrame(() => {
                __raf_id = null;
                dioxus.send(__raf_pending);
            });
        }
    };
    const payload = __PAYLOAD__;
    const rawCleanup = mod.__JS_NAME__(...payload, emit);
    const cleanup = () => {
        if (__raf_id !== null) {
            cancelAnimationFrame(__raf_id);
            __raf_id = null;
        }
        if (typeof rawCleanup === "function") {
            rawCleanup();
        }
    };
    window.__DIOXUS_WATCHERS.set(__SUB_ID__, cleanup);
})();
