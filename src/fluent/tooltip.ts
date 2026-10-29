// v-tip: WinUI ToolTip. One shared element; ~600 ms delay, instant when moving between
// targets; hides on pointer leave/down, scroll, wheel and Escape.
import type { Directive } from 'vue';

type TipPlacement = 'top' | 'bottom' | 'left' | 'right';
type TipValue = string | { text: string; placement?: TipPlacement } | undefined | null;

interface TipState {
  text: string;
  placement: TipPlacement;
  timer: number;
  off: () => void;
}

const SHOW_DELAY = 600;
const WARM_MS = 400;
const states = new WeakMap<HTMLElement, TipState>();
let tipEl: HTMLDivElement | null = null;
let current: HTMLElement | null = null;
let lastHide = 0;

function el(): HTMLDivElement {
  if (tipEl) return tipEl;
  tipEl = document.createElement('div');
  tipEl.className = 'f-tooltip';
  tipEl.id = 'f-tooltip';
  tipEl.setAttribute('role', 'tooltip');
  Object.assign(tipEl.style, {
    position: 'fixed', zIndex: '1300', left: '0', top: '0', maxWidth: '320px',
    padding: '5px 8px 7px', borderRadius: '4px', fontSize: '12px', lineHeight: '16px',
    background: 'var(--tooltip-fill)', color: 'var(--text-primary)',
    border: '1px solid var(--surface-stroke-flyout)', boxShadow: 'var(--shadow-tooltip)',
    pointerEvents: 'none', whiteSpace: 'pre-line', overflowWrap: 'anywhere', display: 'none',
    fontFamily: 'var(--font-ui)',
  } as Partial<CSSStyleDeclaration>);
  document.body.appendChild(tipEl);
  window.addEventListener('scroll', hide, true);
  window.addEventListener('wheel', hide, { passive: true, capture: true });
  window.addEventListener('blur', hide);
  window.addEventListener('keydown', (e) => e.key === 'Escape' && hide(), true);
  return tipEl;
}

function normalize(v: TipValue): { text: string; placement: TipPlacement } {
  if (!v) return { text: '', placement: 'top' };
  if (typeof v === 'string') return { text: v, placement: 'top' };
  return { text: v.text ?? '', placement: v.placement ?? 'top' };
}

function show(target: HTMLElement) {
  const s = states.get(target);
  if (!s?.text || !target.isConnected) return;
  const t = el();
  t.textContent = s.text;
  t.style.display = 'block';
  const r = target.getBoundingClientRect();
  const w = t.offsetWidth, h = t.offsetHeight, gap = 8, m = 4;
  let left: number, top: number;
  let p = s.placement;
  if (p === 'top' && r.top - h - gap < m) p = 'bottom';
  else if (p === 'bottom' && r.bottom + h + gap > innerHeight - m) p = 'top';
  if (p === 'top' || p === 'bottom') {
    left = r.left + r.width / 2 - w / 2;
    top = p === 'top' ? r.top - h - gap : r.bottom + gap;
  } else {
    top = r.top + r.height / 2 - h / 2;
    left = p === 'left' ? r.left - w - gap : r.right + gap;
  }
  t.style.left = `${Math.max(m, Math.min(left, innerWidth - w - m))}px`;
  t.style.top = `${Math.max(m, Math.min(top, innerHeight - h - m))}px`;
  current = target;
  target.setAttribute('aria-describedby', 'f-tooltip');
}

function hide() {
  if (current) {
    const s = states.get(current);
    if (s) clearTimeout(s.timer);
    current.removeAttribute('aria-describedby');
    current = null;
    lastHide = Date.now();
  }
  if (tipEl) tipEl.style.display = 'none';
}

function schedule(target: HTMLElement) {
  const s = states.get(target);
  if (!s?.text) return;
  clearTimeout(s.timer);
  const warm = Date.now() - lastHide < WARM_MS || current !== null;
  if (current && current !== target) hide();
  s.timer = window.setTimeout(() => show(target), warm ? 0 : SHOW_DELAY);
}

function cancel(target: HTMLElement) {
  const s = states.get(target);
  if (s) clearTimeout(s.timer);
  if (current === target) hide();
}

export const vTip: Directive<HTMLElement, TipValue> = {
  mounted(target, binding) {
    const { text, placement } = normalize(binding.value);
    const onEnter = () => schedule(target);
    const onLeave = () => cancel(target);
    const onFocus = () => target.matches(':focus-visible') && schedule(target);
    target.addEventListener('pointerenter', onEnter);
    target.addEventListener('pointerleave', onLeave);
    target.addEventListener('pointerdown', onLeave);
    target.addEventListener('focus', onFocus);
    target.addEventListener('blur', onLeave);
    states.set(target, {
      text,
      placement,
      timer: 0,
      off: () => {
        target.removeEventListener('pointerenter', onEnter);
        target.removeEventListener('pointerleave', onLeave);
        target.removeEventListener('pointerdown', onLeave);
        target.removeEventListener('focus', onFocus);
        target.removeEventListener('blur', onLeave);
      },
    });
  },
  updated(target, binding) {
    const s = states.get(target);
    if (!s) return;
    const { text, placement } = normalize(binding.value);
    s.text = text;
    s.placement = placement;
    if (current === target) {
      if (text) show(target);
      else hide();
    }
  },
  beforeUnmount(target) {
    const s = states.get(target);
    if (!s) return;
    cancel(target);
    s.off();
    states.delete(target);
  },
};
