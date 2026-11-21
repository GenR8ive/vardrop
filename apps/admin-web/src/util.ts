// küçük yardımcılar
export function humanSize(size: number): string {
  const kb = 1024;
  const mb = kb * 1024;
  const gb = mb * 1024;
  if (size < kb) return `${size} B`;
  if (size < mb) return `${(size / kb).toFixed(1)} KB`;
  if (size < gb) return `${(size / mb).toFixed(1)} MB`;
  return `${(size / gb).toFixed(2)} GB`;
}

export function fmtTime(ms?: number): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleString();
}

export function fmtDay(ms?: number): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleDateString();
}
