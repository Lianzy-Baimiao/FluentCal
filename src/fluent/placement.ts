// Popup positioning (flip + shift to stay in the viewport) and the stack of open popups
// used for light dismiss and Escape handling across flyouts, menus and dialogs.

export type Placement =
  | 'bottom-start' | 'bottom-end' | 'bottom'
  | 'top-start' | 'top-end' | 'top'
  | 'right-start' | 'left-start';

export type Anchor = HTMLElement | DOMRect | { x: number; y: number } | null | undefined;

export interface Rect { left: number; top: number; width: number; height: number }

const MARGIN = 8;

export function anchorRect(a: Anchor): Rect {
  if (!a) return { left: innerWidth / 2, top: innerHeight / 3, width: 0, height: 0 };
  if (a instanceof HTMLElement) {
    const r = a.getBoundingClientRect();
    return { left: r.left, top: r.top, width: r.width, height: r.height };
  }
  if ('width' in a) return { left: a.left, top: a.top, width: a.width, height: a.height };
  return { left: a.x, top: a.y, width: 0, height: 0 };
}

export interface Placed { left: number; top: number; side: 'top' | 'bottom' | 'left' | 'right' }

/** Positions a box of size `w`×`h` next to `a`, flipping to the other side and shifting
 *  along the cross axis so it stays inside the viewport. */
export function place(a: Rect, w: number, h: number, placement: Placement, offset = 4): Placed {
  const vw = innerWidth, vh = innerHeight;
  const [side0, align = ''] = placement.split('-') as [Placed['side'], string?];
  let side = side0;
  let left: number, top: number;
  if (side === 'bottom' || side === 'top') {
    const below = vh - (a.top + a.height) - offset - MARGIN;
    const above = a.top - offset - MARGIN;
    if (side === 'bottom' && h > below && above > below) side = 'top';
    else if (side === 'top' && h > above && below > above) side = 'bottom';
    top = side === 'bottom' ? a.top + a.height + offset : a.top - offset - h;
    left = align === 'start' ? a.left : align === 'end' ? a.left + a.width - w : a.left + (a.width - w) / 2;
  } else {
    const right = vw - (a.left + a.width) - offset - MARGIN;
    const leftSpace = a.left - offset - MARGIN;
    if (side === 'right' && w > right && leftSpace > right) side = 'left';
    else if (side === 'left' && w > leftSpace && right > leftSpace) side = 'right';
    left = side === 'right' ? a.left + a.width + offset : a.left - offset - w;
    top = a.top;
  }
  left = Math.max(MARGIN, Math.min(left, vw - w - MARGIN));
  top = Math.max(MARGIN, Math.min(top, vh - h - MARGIN));
  return { left, top, side };
}

// ---------------------------------------------------------------------------------------
// Open popup stack: an outside pointerdown closes only popups above the one that contains
// the target, Escape closes the topmost one.

interface PopupEntry { el: () => HTMLElement | null | undefined; close: () => void; anchor?: () => Element | null | undefined }

const stack: PopupEntry[] = [];

export function pushPopup(e: PopupEntry): () => void {
  stack.push(e);
  ensureListeners();
  return () => {
    const i = stack.indexOf(e);
    if (i >= 0) stack.splice(i, 1);
  };
}

export function isTopPopup(e: PopupEntry): boolean {
  return stack[stack.length - 1] === e;
}

export function closeAllPopups(): void {
  [...stack].reverse().forEach((p) => p.close());
}

let listening = false;
function ensureListeners() {
  if (listening) return;
  listening = true;
  window.addEventListener('pointerdown', (ev) => {
    const target = ev.target as Node | null;
    if (!target) return;
    // Find the deepest popup that contains the target; close everything above it.
    let keep = -1;
    stack.forEach((p, i) => {
      if (p.el()?.contains(target) || p.anchor?.()?.contains(target)) keep = i;
    });
    for (let i = stack.length - 1; i > keep; i--) stack[i]?.close();
  }, true);
  window.addEventListener('keydown', (ev) => {
    if (ev.key === 'Escape' && stack.length) {
      ev.preventDefault();
      ev.stopPropagation();
      stack[stack.length - 1].close();
    }
  }, true);
  window.addEventListener('blur', () => {
    // A native colour picker or file dialog blurs the window; keep the popup that owns it.
    if (document.activeElement instanceof HTMLInputElement && document.activeElement.type === 'color') return;
    closeAllPopups();
  });
  window.addEventListener('resize', closeAllPopups);
}

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement);
}
