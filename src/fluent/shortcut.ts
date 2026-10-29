// Keyboard shortcut strings in the format the Tauri global-shortcut plugin parses,
// e.g. "Ctrl+Alt+D". Modifier order: Ctrl, Alt, Shift, Super.

export const MODIFIERS = ['Ctrl', 'Alt', 'Shift', 'Super'] as const;

const PUNCT: Record<string, string> = {
  Minus: 'Minus', Equal: 'Equal', BracketLeft: 'BracketLeft', BracketRight: 'BracketRight',
  Backslash: 'Backslash', Semicolon: 'Semicolon', Quote: 'Quote', Backquote: 'Backquote',
  Comma: 'Comma', Period: 'Period', Slash: 'Slash',
};

const NAMED: Record<string, string> = {
  Space: 'Space', Enter: 'Enter', Tab: 'Tab', Escape: 'Escape', Backspace: 'Backspace',
  Delete: 'Delete', Insert: 'Insert', Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown',
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
  PrintScreen: 'PrintScreen', Pause: 'Pause', ScrollLock: 'ScrollLock',
};

/** Maps KeyboardEvent.code to a key token, or null for modifiers/unsupported keys. */
export function codeToKey(code: string): string | null {
  let m: RegExpExecArray | null;
  if ((m = /^Key([A-Z])$/.exec(code))) return m[1];
  if ((m = /^Digit(\d)$/.exec(code))) return m[1];
  if ((m = /^F(\d{1,2})$/.exec(code)) && +m[1] >= 1 && +m[1] <= 24) return `F${m[1]}`;
  if ((m = /^Numpad(\d)$/.exec(code))) return `Num${m[1]}`;
  if (code in PUNCT) return PUNCT[code];
  if (code in NAMED) return NAMED[code];
  return null;
}

/** Builds "Ctrl+Alt+D" from a keydown event; null until a non-modifier key is pressed
 *  together with at least one modifier. */
export function shortcutFromEvent(e: KeyboardEvent): string | null {
  const mods = modsOf(e);
  const key = codeToKey(e.code);
  if (!key || !mods.length) return null;
  return [...mods, key].join('+');
}

export function modsOf(e: { ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }): string[] {
  const out: string[] = [];
  if (e.ctrlKey) out.push('Ctrl');
  if (e.altKey) out.push('Alt');
  if (e.shiftKey) out.push('Shift');
  if (e.metaKey) out.push('Super');
  return out;
}

const LABELS: Record<string, string> = {
  Super: 'Win', Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Backslash: '\\',
  Semicolon: ';', Quote: "'", Backquote: '`', Comma: ',', Period: '.', Slash: '/',
  Up: '↑', Down: '↓', Left: '←', Right: '→', Escape: 'Esc', Delete: 'Del', PageUp: 'PgUp', PageDown: 'PgDn',
};

/** Keycap labels for display, e.g. "Ctrl+Alt+D" → ["Ctrl", "Alt", "D"]. */
export function shortcutLabels(s: string): string[] {
  return s ? s.split('+').filter(Boolean).map((t) => LABELS[t] ?? t) : [];
}
