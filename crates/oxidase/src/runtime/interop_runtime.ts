// Dioxus JS Interop - Geometry Runtime Subsystem (TypeScript)
// Lightweight cross-platform DOM measurement and viewport queries

export interface Rect {
    x: number;
    y: number;
    width: number;
    height: number;
    top: number;
    right: number;
    bottom: number;
    left: number;
}

export interface Viewport {
    width: number;
    height: number;
    scrollX: number;
    scrollY: number;
}

/**
 * Measures the bounding client rectangle of an element by ID.
 */
export function measureRect(elementId: string): Rect | null {
    if (typeof document === "undefined") {
        return null;
    }
    const el = document.getElementById(elementId);
    if (!(el instanceof HTMLElement)) {
        return null;
    }
    const r = el.getBoundingClientRect();
    return {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
        left: r.left,
    };
}

/**
 * Queries current viewport dimensions and scroll offsets.
 */
export function getViewport(): Viewport {
    if (typeof window === "undefined") {
        return { width: 0, height: 0, scrollX: 0, scrollY: 0 };
    }
    return {
        width: window.innerWidth,
        height: window.innerHeight,
        scrollX: window.scrollX || window.pageXOffset || 0,
        scrollY: window.scrollY || window.pageYOffset || 0,
    };
}
