import { describe, expect, it } from 'vitest';
import {
  applyTheme, bestTextOn, contrast, deriveAccent, mix, normalizeHex, TASK_COLORS, taskColor, WIN_ACCENTS, withAlpha,
} from './theme';

function lightness(hex: string): number {
  const v = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  return (Math.max(...v) + Math.min(...v)) / 2;
}

describe('deriveAccent', () => {
  it('returns 7 shades ordered from lightest to darkest', () => {
    for (const base of ['#0078D4', '#FFB900', '#107C10', '#8764B8', '#767676']) {
      const p = deriveAccent(base);
      expect(p).toHaveLength(7);
      expect(p[3]).toBe(base);
      for (let i = 1; i < 7; i++) expect(lightness(p[i])).toBeLessThan(lightness(p[i - 1]));
    }
  });

  it('is close to the Windows palette for the default blue', () => {
    // Windows: light3 #99EBFF light2 #4CC2FF light1 #0091F8 dark1 #0067C0 dark2 #003E92 dark3 #001A68
    const win = ['#99EBFF', '#4CC2FF', '#0091F8', '#0078D4', '#0067C0', '#003E92', '#001A68'];
    deriveAccent('#0078D4').forEach((c, i) => {
      expect(Math.abs(lightness(c) - lightness(win[i]))).toBeLessThan(0.03);
    });
  });
});

describe('colour math', () => {
  it('computes WCAG contrast', () => {
    expect(contrast('#000000', '#FFFFFF')).toBeCloseTo(21, 5);
    expect(contrast('#777777', '#777777')).toBeCloseTo(1, 5);
  });

  it('mixes linearly', () => {
    expect(mix('#000000', '#FFFFFF', 0)).toBe('#000000');
    expect(mix('#000000', '#FFFFFF', 1)).toBe('#FFFFFF');
    expect(mix('#000000', '#FFFFFF', 0.5)).toBe('#808080');
  });

  it('normalizes hex and adds alpha', () => {
    expect(normalizeHex('#abc')).toBe('#AABBCC');
    expect(normalizeHex('0078d4')).toBe('#0078D4');
    expect(normalizeHex('nope')).toBeNull();
    expect(withAlpha('#0078D4', 0.5)).toBe('#0078D480');
  });

  it('keeps white text on mid-tone accents, black on light ones', () => {
    expect(bestTextOn('#0078D4')).toBe('#FFFFFF');
    expect(bestTextOn('#FFB900')).toBe('#000000');
    expect(bestTextOn('#4CC2FF')).toBe('#000000');
  });
});

describe('task colours', () => {
  it('meets 4.5:1 for text on both themes and for text on bars', () => {
    for (const c of TASK_COLORS) {
      expect(contrast(c.textLight, '#FFFFFF'), `${c.key} light`).toBeGreaterThanOrEqual(4.5);
      expect(contrast(c.textDark, '#202020'), `${c.key} dark`).toBeGreaterThanOrEqual(4.5);
      const on = bestTextOn(c.bar);
      expect(contrast(on, c.bar), `${c.key} bar`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('resolves keys, custom hex and the default', () => {
    expect(taskColor('red', false).text).toBe('#C42B1C');
    expect(taskColor('red', true).text).toBe('#FF99A4');
    const custom = taskColor('#FFFF00', false);
    expect(custom.bar).toBe('#FFFF00');
    expect(contrast(custom.text, '#FFFFFF')).toBeGreaterThanOrEqual(4.5);
    expect(taskColor(undefined, true).bar).toBe('var(--accent-fill)');
  });

  it('lists the 48 Windows accents', () => {
    expect(WIN_ACCENTS).toHaveLength(48);
    expect(new Set(WIN_ACCENTS).size).toBe(48);
  });
});

describe('applyTheme', () => {
  it('sets theme attributes and accent variables', () => {
    const props = new Map<string, string>();
    const el = {
      dataset: {} as Record<string, string>,
      style: { colorScheme: '', setProperty: (k: string, v: string) => props.set(k, v) },
    } as unknown as HTMLElement;
    applyTheme(el, { dark: true, accent: '#FFB900' });
    expect(el.dataset.theme).toBe('dark');
    expect(props.get('--accent-base')).toBe('#FFB900');
    expect(props.get('--text-on-accent-dark')).toBe('#000000');
    const win = ['#99EBFF', '#4CC2FF', '#0091F8', '#0078D4', '#0067C0', '#003E92', '#001A68'];
    applyTheme(el, { dark: false, accent: '#0078D4', palette: win });
    expect(props.get('--accent-light2')).toBe('#4CC2FF');
    expect(props.get('--text-on-accent-light')).toBe('#FFFFFF');
  });
});
