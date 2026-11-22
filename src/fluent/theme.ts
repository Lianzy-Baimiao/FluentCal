// Theme helpers: accent palette derivation, WCAG contrast, task colour palette.
import { colorOpacity, compositeColor, normalizeRgba, opaqueColor, setColorOpacity } from '@/lib/rgba';

type RGB = [number, number, number];

export function normalizeHex(hex: string): string | null {
  return normalizeRgba(hex);
}

function toRgb(hex: string): RGB {
  const h = normalizeHex(hex) ?? '#000000';
  return [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)) as RGB;
}

function toHex([r, g, b]: RGB): string {
  return `#${[r, g, b].map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, '0')).join('').toUpperCase()}`;
}

function rgbToHsl([r, g, b]: RGB): [number, number, number] {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

function hslToRgb(h: number, s: number, l: number): RGB {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [f(0) * 255, f(8) * 255, f(4) * 255];
}

/** Windows-like accent shades: [light3, light2, light1, base, dark1, dark2, dark3].
 *  Lightness moves a fixed share of the remaining distance to white/black, calibrated
 *  against the default Windows 11 blue palette. */
export function deriveAccent(hex: string): string[] {
  const [h, s, l] = rgbToHsl(toRgb(hex));
  const up = [0.658, 0.399, 0.121];
  const down = [0.094, 0.311, 0.509];
  const lighter = up.map((t) => hslToRgb(h, s, l + (1 - l) * t)).map(toHex);
  const darker = down.map((t) => hslToRgb(h, s, l * (1 - t))).map(toHex);
  return [...lighter, opaqueColor(normalizeHex(hex) ?? '#0078D4'), ...darker];
}

function luminance(hex: string): number {
  const [r, g, b] = toRgb(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2 contrast ratio between two opaque colours. */
export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** Linear blend from `a` (t = 0) to `b` (t = 1). */
export function mix(a: string, b: string, t: number): string {
  const p = toRgb(a), q = toRgb(b);
  return toHex([0, 1, 2].map((i) => p[i] + (q[i] - p[i]) * t) as RGB);
}

/** `#RRGGBB` + alpha (0..1) → `#RRGGBBAA`. */
export function withAlpha(hex: string, alpha: number): string {
  const h = opaqueColor(normalizeHex(hex) ?? '#000000');
  return h + Math.round(Math.min(1, Math.max(0, alpha)) * 255).toString(16).padStart(2, '0').toUpperCase();
}

/** Fluent accent foreground: prefer AA white on mid-tone accents, otherwise use the better contrast. */
export function bestTextOn(fill: string, background = '#FFFFFF'): string {
  fill = compositeColor(fill, background);
  const white = contrast(fill, '#FFFFFF');
  return white >= 4.5 || white >= contrast(fill, '#000000') ? '#FFFFFF' : '#000000';
}

export function bestMonochromeText(fill: string): string {
  const color = compositeColor(fill, '#FFFFFF');
  return contrast(color, '#000000') >= contrast(color, '#FFFFFF') ? '#000000' : '#FFFFFF';
}

export function accentTextOn(text: string, background: string): string {
  const base = opaqueColor(text);
  const target = contrast(base, '#FFFFFF') >= contrast(base, '#000000') ? '#FFFFFF' : '#000000';
  for (let step = 1; step <= 10; step++) {
    const adjusted = mix(base, target, step / 10);
    if (contrast(adjusted, background) >= 4.5) return adjusted;
  }
  return target;
}

export interface ThemeInput {
  dark: boolean;
  accent: string;
  /** 7 colours from Windows (light3 … dark3) when available. */
  palette?: string[];
}

const ACCENT_VARS = ['light3', 'light2', 'light1', 'base', 'dark1', 'dark2', 'dark3'];

export function applyTheme(root: HTMLElement, t: ThemeInput): void {
  const palette = t.palette?.length === 7 && t.palette.every((c) => normalizeHex(c))
    ? t.palette.map((c) => normalizeHex(c)!)
    : deriveAccent(t.accent);
  root.dataset.theme = t.dark ? 'dark' : 'light';
  root.style.colorScheme = t.dark ? 'dark' : 'light';
  ACCENT_VARS.forEach((name, i) => root.style.setProperty(`--accent-${name}`, palette[i]));
  const opacity = t.palette?.length === 7 ? 1 : colorOpacity(t.accent);
  const fill = palette[t.dark ? 1 : 4];
  root.style.setProperty('--accent-fill', setColorOpacity(fill, opacity));
  root.style.setProperty('--accent-fill-hover', setColorOpacity(fill, opacity * 0.9));
  root.style.setProperty('--accent-fill-pressed', setColorOpacity(fill, opacity * 0.8));
  root.style.setProperty('--attention', setColorOpacity(fill, opacity));
  root.style.setProperty('--accent-alpha', String(opacity));
  // Light theme fills with dark1, dark theme with light2 (WinUI AccentFillColorDefault).
  root.style.setProperty('--text-on-accent-light', bestTextOn(setColorOpacity(palette[4], opacity), '#FFFFFF'));
  root.style.setProperty('--text-on-accent-dark', bestTextOn(setColorOpacity(palette[1], opacity), '#202020'));
  const accentText = accentTextOn(palette[t.dark ? 0 : 5], t.dark ? '#202020' : '#FFFFFF');
  root.style.setProperty('--accent-text', accentText);
  root.style.setProperty('--accent-text-secondary', accentTextOn(palette[t.dark ? 0 : 6], t.dark ? '#202020' : '#FFFFFF'));
  root.style.setProperty('--accent-text-light', accentTextOn(palette[5], '#FFFFFF'));
  root.style.setProperty('--accent-text-dark', accentTextOn(palette[0], '#202020'));
}

export interface TaskColor { key: string; label: string; bar: string; textLight: string; textDark: string }

/** Task palette. `bar` carries white (or black for yellow/gray) text; textLight/textDark
 *  meet 4.5:1 on #FFFFFF / #202020. */
export const TASK_COLORS: TaskColor[] = [
  { key: 'red', label: '红', bar: '#D13438', textLight: '#C42B1C', textDark: '#FF99A4' },
  { key: 'orange', label: '橙', bar: '#B5470D', textLight: '#B5470D', textDark: '#FFB27D' },
  { key: 'yellow', label: '黄', bar: '#FFB900', textLight: '#8A6100', textDark: '#FCE100' },
  { key: 'green', label: '绿', bar: '#107C10', textLight: '#0F7B0F', textDark: '#6CCB5F' },
  { key: 'teal', label: '青', bar: '#03787C', textLight: '#03787C', textDark: '#4FD6D9' },
  { key: 'blue', label: '蓝', bar: '#0078D4', textLight: '#0063B1', textDark: '#60CDFF' },
  { key: 'indigo', label: '靛', bar: '#4F52B2', textLight: '#4F52B2', textDark: '#A8AAF2' },
  { key: 'purple', label: '紫', bar: '#8764B8', textLight: '#744DA9', textDark: '#D0B3F5' },
  { key: 'pink', label: '粉', bar: '#C239B3', textLight: '#AD2E9F', textDark: '#F7A1E9' },
  { key: 'gray', label: '灰', bar: '#7A7574', textLight: '#5D5A58', textDark: '#BDB8B6' },
];

/** Resolve a task colour key (or custom hex) for the current theme. `undefined` uses
 *  the theme's default text and the accent fill for bars. */
export function taskColor(key: string | undefined, dark: boolean): { bar: string; text: string; onBar: string } {
  if (key) {
    const c = TASK_COLORS.find((t) => t.key === key);
    if (c) return { bar: c.bar, text: dark ? c.textDark : c.textLight, onBar: bestTextOn(c.bar) };
    const hex = normalizeHex(key);
    if (hex) {
      const bg = dark ? '#202020' : '#FFFFFF';
      let text = opaqueColor(hex);
      for (let i = 1; i <= 10 && contrast(text, bg) < 4.5; i++) text = mix(hex, dark ? '#FFFFFF' : '#000000', i * 0.1);
      return { bar: hex, text, onBar: bestTextOn(hex, bg) };
    }
  }
  return {
    bar: 'var(--accent-fill)',
    text: 'var(--text-primary)',
    onBar: 'var(--text-on-accent)',
  };
}

export function progressShade(hex: string): string {
  const match = normalizeHex(hex);
  if (!match) return hex;
  const channels = match.slice(1).match(/.{2}/g) ?? [];
  const rgb = channels.slice(0, 3).map(c => Math.round(parseInt(c, 16) * 0.78).toString(16).padStart(2, '0')).join('').toUpperCase();
  return `#${rgb}${channels.length === 4 ? channels[3] : ''}`;
}

/** The 48 Windows 11 accent colours, in Settings order. */
export const WIN_ACCENTS: string[] = [
  'FFB900', 'FF8C00', 'F7630C', 'CA5010', 'DA3B01', 'EF6950', 'D13438', 'FF4343',
  'E74856', 'E81123', 'EA005E', 'C30052', 'E3008C', 'BF0077', 'C239B3', '9A0089',
  '0078D4', '0063B1', '8E8CD8', '6B69D6', '8764B8', '744DA9', 'B146C2', '881798',
  '0099BC', '2D7D9A', '00B7C3', '038387', '00B294', '018574', '00CC6A', '10893E',
  '7A7574', '5D5A58', '68768A', '515C6B', '567C73', '486860', '498205', '107C10',
  '767676', '4C4A48', '69797E', '4A5459', '647C64', '525E54', '847545', '7E735F',
].map((c) => `#${c}`);
