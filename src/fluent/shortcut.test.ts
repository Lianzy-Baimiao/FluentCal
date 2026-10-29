import { describe, expect, it } from 'vitest';
import { codeToKey, shortcutFromEvent, shortcutLabels } from './shortcut';

const ev = (code: string, m: Partial<KeyboardEvent> = {}) =>
  ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...m }) as KeyboardEvent;

describe('shortcuts', () => {
  it('maps key codes', () => {
    expect(codeToKey('KeyD')).toBe('D');
    expect(codeToKey('Digit1')).toBe('1');
    expect(codeToKey('F12')).toBe('F12');
    expect(codeToKey('ArrowUp')).toBe('Up');
    expect(codeToKey('ControlLeft')).toBeNull();
  });

  it('needs a modifier and a key', () => {
    expect(shortcutFromEvent(ev('KeyD', { ctrlKey: true, altKey: true }))).toBe('Ctrl+Alt+D');
    expect(shortcutFromEvent(ev('KeyN', { shiftKey: true, ctrlKey: true }))).toBe('Ctrl+Shift+N');
    expect(shortcutFromEvent(ev('KeyD'))).toBeNull();
    expect(shortcutFromEvent(ev('AltLeft', { altKey: true }))).toBeNull();
  });

  it('formats labels', () => {
    expect(shortcutLabels('Ctrl+Alt+D')).toEqual(['Ctrl', 'Alt', 'D']);
    expect(shortcutLabels('Super+Shift+Up')).toEqual(['Win', 'Shift', '↑']);
    expect(shortcutLabels('')).toEqual([]);
  });
});
