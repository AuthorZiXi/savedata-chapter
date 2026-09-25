export function fmtSize(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

export function displayPath(p: { name: string | null; path: string }): string {
    return p.name ? `${p.name}（${p.path}）` : p.path;
}