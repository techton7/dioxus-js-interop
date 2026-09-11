// Test bridge TypeScript file for dioxus-js-bindgen verification

/**
 * Focus an element by ID (Command)
 */
export function focusElement(id: string): void {
    const el = document.getElementById(id);
    if (el) el.focus();
}

/**
 * Scroll to position (Command with multiple params)
 */
export function scrollToPosition(x: number, y: number): void {
    window.scrollTo(x, y);
}

export interface Rect {
    x: number;
    y: number;
    width: number;
    height: number;
}

/**
 * Get bounding client rect (Query with return type)
 */
export function getBoundingRect(id: string): Rect {
    const el = document.getElementById(id);
    if (!el) throw new Error("Element not found: " + id);
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
}

/**
 * Async query returning Promise<string>
 */
export async function fetchRemoteTitle(url: string): Promise<string> {
    return "Title for " + url;
}

export interface WindowSize {
    width: number;
    height: number;
}

/**
 * Watch window resize events (Watcher)
 * #[watcher]
 */
export function watchResize(emit: (size: WindowSize) => void): () => void {
    const handler = () => emit({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", handler);
    return () => window.removeEventListener("resize", handler);
}

/**
 * Higher-order cleanup return without #[watcher] comment in TS
 */
export function watchCustomSignal(multiplier: number, emit: (val: number) => void): () => void {
    const handler = () => emit(42 * multiplier);
    return () => {};
}

/**
 * Test Array generic parameter and return
 */
export function processTags(tags: Array<string>): Array<string> {
    return tags.map(t => t.toUpperCase());
}

/**
 * Test Array generic with Promise async Query
 */
export async function fetchScores(ids: Array<string>): Promise<Array<number>> {
    return ids.map(id => id.length);
}

