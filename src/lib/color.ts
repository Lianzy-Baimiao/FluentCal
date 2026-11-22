import { normalizeRgba, parseRgba } from './rgba';
/** RGB channels of any supported hex; invalid input gives mid gray. */
export function hexToRgb(hex: string): [number, number, number] {
  const c = parseRgba(hex);
  return c ? [c[0], c[1], c[2]] : [128, 128, 128];
}

export function rgbToHex(r: number, g: number, b: number): string {
  const c = (v: number) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0');
  return `#${c(r)}${c(g)}${c(b)}`.toUpperCase();
}

export function alpha(hex: string, a: number): string {
  const [r, g, b] = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${Math.max(0, Math.min(1, a * (parseRgba(hex)?.[3] ?? 1))).toFixed(3)})`;
}

/** Linear mix: t = 0 → a, t = 1 → b. */
export function mixHex(a: string, b: string, t: number): string {
  const x = hexToRgb(a);
  const y = hexToRgb(b);
  return rgbToHex(x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t);
}

export function isHex(s: string | undefined): s is string {
  return !!s && s.startsWith('#') && normalizeRgba(s) !== null;
}
