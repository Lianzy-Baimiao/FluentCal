/** Shared CSS-order RGBA: alpha is last, 00 transparent and FF opaque. */
export type RGBA = [number, number, number, number];

export function normalizeRgba(value: string): string | null {
  const match = /^#?([\da-f]{3}|[\da-f]{4}|[\da-f]{6}|[\da-f]{8})$/i.exec(value.trim());
  if (!match) return null;
  const h = match[1].length < 5 ? [...match[1]].map(c => c + c).join('') : match[1];
  return '#' + h.toUpperCase();
}

export function parseRgba(value: string): RGBA | null {
  const css = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)$/i.exec(value.trim());
  if (css) {
    return [Number(css[1]), Number(css[2]), Number(css[3]), css[4] === undefined ? 1 : Math.max(0, Math.min(1, Number(css[4])))];
  }
  const h = normalizeRgba(value);
  if (!h) return null;
  return [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16), h.length === 9 ? parseInt(h.slice(7, 9), 16) / 255 : 1];
}

export function rgbaHex([r, g, b, a]: RGBA): string {
  const byte = (v: number) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0').toUpperCase();
  return `#${byte(r)}${byte(g)}${byte(b)}` + (a < 1 ? byte(a * 255) : '');
}

export function colorOpacity(value: string): number { return parseRgba(value)?.[3] ?? 1; }

/** Replace alpha (picker); multiplying an existing layer is a separate operation. */
export function setColorOpacity(value: string, opacity: number): string {
  const c = parseRgba(value) ?? [0, 0, 0, 1];
  return rgbaHex([c[0], c[1], c[2], Math.max(0, Math.min(1, opacity))]);
}

export function opaqueColor(value: string): string { return setColorOpacity(value, 1); }

export function compositeColor(fill: string, background: string): string {
  const c = parseRgba(fill) ?? [0, 0, 0, 1];
  const bg = parseRgba(background) ?? [255, 255, 255, 1];
  const a = c[3] + bg[3] * (1 - c[3]);
  if (!a) return '#00000000';
  return rgbaHex([(c[0]*c[3]+bg[0]*bg[3]*(1-c[3]))/a, (c[1]*c[3]+bg[1]*bg[3]*(1-c[3]))/a, (c[2]*c[3]+bg[2]*bg[3]*(1-c[3]))/a, a]);
}
