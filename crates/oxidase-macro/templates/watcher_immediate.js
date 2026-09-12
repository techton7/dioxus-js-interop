(function() {
    const mod = window.__DIOXUS_BINDGEN_MODULES__?.["__MODULE_HASH__"];
    if (!mod) {
        console.error("[oxidase]: Module '__MODULE_HASH__' not found. Cannot start watcher.");
        dioxus.send({ __bindgen_err: "MODULE_NOT_FOUND" });
        return;
    }
    if (!window.__DIOXUS_WATCHERS) window.__DIOXUS_WATCHERS = new Map();
    const emit = (val) => dioxus.send(val);
    const payload = __PAYLOAD__;
    const cleanup = mod.__JS_NAME__(...payload, emit);
    window.__DIOXUS_WATCHERS.set(__SUB_ID__, cleanup);
})();
