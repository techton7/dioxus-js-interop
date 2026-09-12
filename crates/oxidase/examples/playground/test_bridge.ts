// Bridge for live runtime verification of oxidase in standalone playground

/**
 * Focus element by ID (Command)
 */
export function focusElement(id: string): void {
    const el = document.getElementById(id);
    if (!el) throw new Error("Element not found: " + id);
    el.focus();
}

/**
 * Failing command (Command)
 */
export function failCommand(id: string): void {
    const el = document.getElementById(id);
    if (!el) throw new Error("Intentional command error for id: " + id);
}

export interface BoxRect {
    x: number;
    y: number;
    width: number;
    height: number;
}

/**
 * Get bounding client rect (Query)
 */
export function getBoundingRect(id: string): BoxRect {
    const el = document.getElementById(id);
    if (!el) throw new Error("Query failed: element not found: " + id);
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
}

export interface WindowMetrics {
    width: number;
    height: number;
}

/**
 * Watch window resize events (Watcher)
 * #[watcher]
 */
export function watchWindowResize(emit: (metrics: WindowMetrics) => void): () => void {
    const handler = () => {
        emit({ width: window.innerWidth, height: window.innerHeight });
    };
    window.addEventListener("resize", handler);
    return () => {
        window.removeEventListener("resize", handler);
    };
}

export interface StreamPayload {
    value: number;
    elapsedMs: number;
    isDone: boolean;
}

/**
 * Watcher: High frequency stream over durationMs (default 1000ms) with customizable target count
 * #[watcher]
 */
export function watchImmediateStream(targetCount: number, durationMs: number, emit: (val: StreamPayload) => void): () => void {
    const startTime = performance.now();
    let emittedCount = 0;
    const interval = 10;
    const totalTicks = Math.max(1, Math.round(durationMs / interval));
    let currentTick = 0;

    const timer = setInterval(() => {
        currentTick++;
        const targetByNow = Math.min(targetCount, Math.round((currentTick / totalTicks) * targetCount));
        while (emittedCount < targetByNow) {
            emittedCount++;
            const isDone = emittedCount >= targetCount || currentTick >= totalTicks;
            emit({
                value: emittedCount,
                elapsedMs: performance.now() - startTime,
                isDone,
            });
        }
        if (currentTick >= totalTicks || emittedCount >= targetCount) {
            clearInterval(timer);
        }
    }, interval);

    return () => {
        clearInterval(timer);
    };
}

/**
 * Watcher: rAF-coalesced high frequency stream over durationMs (default 1000ms) with customizable target count
 * #[watcher(raf)]
 */
export function watchRafStream(targetCount: number, durationMs: number, emit: (val: StreamPayload) => void): () => void {
    const startTime = performance.now();
    let emittedCount = 0;
    const interval = 10;
    const totalTicks = Math.max(1, Math.round(durationMs / interval));
    let currentTick = 0;

    const timer = setInterval(() => {
        currentTick++;
        const targetByNow = Math.min(targetCount, Math.round((currentTick / totalTicks) * targetCount));
        while (emittedCount < targetByNow) {
            emittedCount++;
            const isDone = emittedCount >= targetCount || currentTick >= totalTicks;
            emit({
                value: emittedCount,
                elapsedMs: performance.now() - startTime,
                isDone,
            });
        }
        if (currentTick >= totalTicks || emittedCount >= targetCount) {
            clearInterval(timer);
        }
    }, interval);

    return () => {
        clearInterval(timer);
    };
}



