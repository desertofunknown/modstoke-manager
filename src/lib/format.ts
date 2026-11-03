export function clampNumber(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

export function cssPixels(value: string): number {
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? parsed : 0;
}

export function formatCount(value?: number): string {
    return typeof value === "number" ? value.toLocaleString() : "-";
}

export function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}
