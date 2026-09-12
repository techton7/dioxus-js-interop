(function() {
    const mod = window.__DIOXUS_BINDGEN_MODULES__?.["__MODULE_HASH__"];
    if (!mod) {
        console.warn("[oxidase]: Module '__MODULE_HASH__' not found. Browser context may have reloaded.");
        return;
    }
    const payload = __PAYLOAD__;
    try {
        mod.__JS_NAME__(...payload);
    } catch (e) {
        console.error("[oxidase Command Error in __JS_NAME__]:", e);
    }
})();
