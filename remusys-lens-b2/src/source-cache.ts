import type { SourceTy } from "remusys-wasm-b2";

const CACHE_KEY = "remusys-lens-b2:last-loaded-source:v1";

export type CachedSource = {
    type: SourceTy;
    text: string;
    filename: string;
};

type CachedPayload = {
    version: 1;
    type: SourceTy;
    text: string;
    filename: string;
};

function canUseStorage(): boolean {
    return typeof window !== "undefined" && typeof window.localStorage !== "undefined";
}

export function loadCachedSource(): CachedSource | null {
    if (!canUseStorage()) return null;
    const raw = window.localStorage.getItem(CACHE_KEY);
    if (!raw) return null;

    try {
        const payload = JSON.parse(raw) as Partial<CachedPayload>;
        if (payload.version !== 1) return null;
        if (payload.type !== "ir" && payload.type !== "sysy") return null;
        if (typeof payload.text !== "string" || typeof payload.filename !== "string") {
            return null;
        }
        return {
            type: payload.type,
            text: payload.text,
            filename: payload.filename,
        };
    } catch {
        return null;
    }
}

export function saveCachedSource(source: CachedSource): void {
    if (!canUseStorage()) return;
    const payload: CachedPayload = {
        version: 1,
        type: source.type,
        text: source.text,
        filename: source.filename,
    };
    window.localStorage.setItem(CACHE_KEY, JSON.stringify(payload));
}

export function clearCachedSource(): void {
    if (!canUseStorage()) return;
    window.localStorage.removeItem(CACHE_KEY);
}
