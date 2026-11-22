import { expect, it } from 'vitest';
import { compositeColor, normalizeRgba, parseRgba, setColorOpacity } from './rgba';

it('normalizes all CSS hex forms, including short alpha', () => {
  expect(normalizeRgba('#abc')).toBe('#AABBCC');
  expect(normalizeRgba('#1A2B')).toBe('#11AA22BB');
  expect(normalizeRgba('#12345680')).toBe('#12345680');
  expect(normalizeRgba('#12345')).toBeNull();
});

it('replaces opacity and composites RGBA over a surface', () => {
  expect(setColorOpacity('#12345680', 0)).toBe('#12345600');
  expect(setColorOpacity('#12345680', 1)).toBe('#123456');
  expect(compositeColor('#FF000080', '#FFFFFF')).toBe('#FF7F7F');
  expect(parseRgba('#12345680')?.[3]).toBeCloseTo(128 / 255);
  expect(compositeColor('rgba(20, 20, 20, 0.4)', '#F3F3F3')).toBe('#9A9A9A');
});
