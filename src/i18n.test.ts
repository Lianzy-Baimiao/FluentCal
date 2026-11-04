import { describe, expect, it } from 'vitest';
import { formatMessage, resolveLanguage, validPreference, language } from './i18n';
import { buildRows, headerTiers, weekLabel } from './lib/gantt';
import { displayInfo, dayInfo } from './lib/lunar';

describe('display language', () => {
  it('follows scripts and regional aliases, with an English fallback', () => {
    expect(resolveLanguage('system', 'zh-Hans-HK')).toBe('zh-CN');
    expect(resolveLanguage('system', 'zh_HK')).toBe('zh-TW');
    expect(resolveLanguage('system', 'zh-Hant-CN')).toBe('zh-TW');
    expect(resolveLanguage('system', 'de-DE')).toBe('en-US');
    expect(resolveLanguage('en-US', 'zh-CN')).toBe('en-US');
    expect(validPreference('bad')).toBe(false);
  });
  it('preserves user text containing placeholder syntax', () => {
    expect(formatMessage('{1}: {0}', ['keep {1}', '例子'])).toBe('例子: keep {1}');
  });
  it('translates generated calendar labels without changing canonical dates or user group names', () => {
    const previous = language.value;
    const canonical = dayInfo('2026-11-03');
    try {
      language.value = 'en-US';
      expect(displayInfo('2026-11-03').lunarLabel).toBe('M9 D25');
      expect(dayInfo('2026-11-03')).toEqual(canonical);
      expect(headerTiers({ start: '2026-11-01', end: '2026-11-30' }, 'day', '2026-11-03', 1).top[0].label).toBe('2026/11');
      expect(weekLabel('2026-11-03')).toBe('Week 45');
      const rows = buildRows([], [{ id: 'user-group', name: '未分组', color: 'blue', order: 0 }], { hideDone: false, hideSingleDay: false, groupFilter: [] });
      expect(rows[0].kind === 'group' && rows[0].group.name).toBe('未分组');
      language.value = 'zh-TW';
      expect(displayInfo('2026-11-03').lunarLabel).toBe('九月廿五');
    } finally { language.value = previous; }
  });
});
