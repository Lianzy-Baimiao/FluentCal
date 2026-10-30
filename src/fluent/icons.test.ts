import { describe, expect, it } from 'vitest';
import { GLYPHS, ICON_NAMES, SVG_ICONS, WEB_SVG } from './icons';

describe('icons', () => {
  it('maps every glyph to one private-use codepoint', () => {
    for (const [name, g] of Object.entries(GLYPHS)) {
      expect(g, name).toHaveLength(1);
      const cp = g.codePointAt(0)!;
      expect(cp >= 0xe000 && cp <= 0xf8ff, `${name} U+${cp.toString(16)}`).toBe(true);
    }
  });

  it('has web outlines only for existing glyphs', () => {
    for (const [name, paths] of Object.entries(WEB_SVG)) {
      expect(name in GLYPHS, name).toBe(true);
      expect(paths!.length, name).toBeGreaterThan(0);
    }
  });

  it('exposes glyph and svg names together without clashes', () => {
    const svg = Object.keys(SVG_ICONS);
    expect(svg.some((n) => n in GLYPHS)).toBe(false);
    expect(ICON_NAMES).toHaveLength(Object.keys(GLYPHS).length + svg.length);
  });
});
