import { describe, expect, it } from 'vitest';
import { GLYPHS, ICON_NAMES, SVG_ICONS } from './icons';

describe('icons', () => {
  it('maps every glyph to one private-use codepoint', () => {
    for (const [name, g] of Object.entries(GLYPHS)) {
      expect(g, name).toHaveLength(1);
      const cp = g.codePointAt(0)!;
      expect(cp >= 0xe000 && cp <= 0xf8ff, `${name} U+${cp.toString(16)}`).toBe(true);
    }
  });

  it('exposes glyph and svg names together without clashes', () => {
    const svg = Object.keys(SVG_ICONS);
    expect(svg.some((n) => n in GLYPHS)).toBe(false);
    expect(ICON_NAMES).toHaveLength(Object.keys(GLYPHS).length + svg.length);
  });
});
