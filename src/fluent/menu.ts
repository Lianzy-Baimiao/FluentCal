import { createApp, markRaw, shallowReactive } from 'vue';
import type { Anchor } from './placement';

export interface MenuItem {
  key?: string;
  label?: string;
  icon?: string;
  shortcut?: string;
  checked?: boolean;
  radio?: boolean;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
  swatches?: { key: string; color: string; title?: string; selected?: boolean }[];
  onSwatch?: (key: string) => void;
  children?: MenuItem[];
  action?: () => void;
}

export type MenuPlacement = 'bottom-start' | 'bottom-end' | 'right-start';

export const menuState = shallowReactive({
  id: 0,
  open: false,
  items: [] as MenuItem[],
  at: null as Anchor,
  placement: 'bottom-start' as MenuPlacement,
  /** Element focused before the menu opened; focus returns there on close. */
  restore: null as HTMLElement | null,
});

let resolver: (() => void) | null = null;
let mounted = false;

async function mount() {
  if (mounted) return;
  mounted = true;
  const { default: MenuHost } = await import('./MenuHost.vue');
  const host = document.createElement('div');
  host.className = 'f-menu-host';
  document.body.appendChild(host);
  createApp(MenuHost).mount(host);
}

/** Opens a WinUI MenuFlyout at a point, rect or element. Resolves when it closes. */
export async function showMenu(
  items: MenuItem[],
  at: { x: number; y: number } | DOMRect | HTMLElement,
  opts?: { placement?: MenuPlacement },
): Promise<void> {
  // Clicking the button that opened the menu again closes it (WinUI flyout behaviour).
  if (menuState.open && at instanceof HTMLElement && menuState.at === at) {
    closeMenu();
    return;
  }
  closeMenu();
  await mount();
  menuState.items = markRaw(items);
  menuState.at = at instanceof HTMLElement ? markRaw(at) : at;
  menuState.placement = opts?.placement ?? 'bottom-start';
  menuState.restore = document.activeElement instanceof HTMLElement ? markRaw(document.activeElement) : null;
  menuState.id++;
  menuState.open = true;
  return new Promise((resolve) => (resolver = resolve));
}

export function closeMenu(): void {
  if (!menuState.open) return;
  menuState.open = false;
  const r = menuState.restore;
  menuState.restore = null;
  if (r?.isConnected) r.focus({ preventScroll: true });
  const done = resolver;
  resolver = null;
  done?.();
}
