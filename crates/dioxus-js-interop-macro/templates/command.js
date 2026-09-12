(function() {
    const mod = window.__DIOXUS_BINDGEN_MODULES__?.["__MODULE_HASH__"];
    if (!mod) {
        console.warn("[dioxus-js-interop]: Module '__MODULE_HASH__' not found. Browser context may have reloaded.");
        return;
    }
    try {
        const payload = __PAYLOAD__;
        mod.__JS_NAME__(...payload);
    } catch (e) {
        console.error("[dioxus-js-interop Command Error in __JS_NAME__]:", e);
    }
})();
