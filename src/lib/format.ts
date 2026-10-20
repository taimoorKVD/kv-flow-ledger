const SYMBOLS: Record<string, string> = {
  USD: "$",
  PKR: "Rs ",
  EUR: "€",
  GBP: "£",
  AED: "AED ",
  SAR: "SAR ",
  INR: "₹",
};

export function money(minor: number, currency = "USD"): string {
  const code = (currency || "USD").toUpperCase();
  const sign = minor < 0 ? "-" : "";
  const abs = Math.abs(minor);
  const whole = Math.floor(abs / 100);
  const frac = abs % 100;
  const symbol = SYMBOLS[code] ?? `${code} `;
  return `${sign}${symbol}${whole.toLocaleString("en-US")}.${frac.toString().padStart(2, "0")}`;
}

export function currencyLabel(code: string): string {
  return (code || "USD").toUpperCase();
}

export function dur(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return `${h}h ${m}m`;
}

export function hms(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const parts = [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60];
  return parts.map((x) => String(x).padStart(2, "0")).join(":");
}

export function compactTokens(n: number | null | undefined, available: boolean): string {
  if (!available || n == null) return "Unavailable";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 1000) return `${Math.round(n / 1000)}K`;
  return String(n);
}

export function margin(pct: number | null | undefined): string {
  if (pct == null || Number.isNaN(pct)) return "N/A";
  return `${pct.toFixed(1)}%`;
}
