import { createApp, h, ref } from 'vue';
import FDialog from './FDialog.vue';

export interface ConfirmOptions {
  title: string;
  content?: string;
  primaryText?: string;
  secondaryText?: string;
  closeText?: string;
  danger?: boolean;
}

export type ConfirmResult = 'primary' | 'secondary' | 'close';

const open = ref(false);
const opts = ref<ConfirmOptions | null>(null);
let resolver: ((r: ConfirmResult) => void) | null = null;
let mounted = false;

function finish(r: ConfirmResult) {
  if (!open.value) return;
  open.value = false;
  const done = resolver;
  resolver = null;
  done?.(r);
}

function mount() {
  if (mounted) return;
  mounted = true;
  const host = document.createElement('div');
  host.className = 'f-dialog-host';
  document.body.appendChild(host);
  createApp({
    render: () =>
      opts.value &&
      h(
        FDialog,
        {
          open: open.value,
          title: opts.value.title,
          primaryText: opts.value.primaryText ?? '确定',
          secondaryText: opts.value.secondaryText,
          closeText: opts.value.closeText ?? '取消',
          danger: opts.value.danger,
          'onUpdate:open': (v: boolean) => !v && finish('close'),
          onPrimary: () => finish('primary'),
          onSecondary: () => finish('secondary'),
        },
        () => (opts.value?.content ? h('p', { style: 'margin:0;white-space:pre-line' }, opts.value.content) : null),
      ),
  }).mount(host);
}

/** Shows a WinUI ContentDialog and resolves with the button that closed it. */
export function confirm(o: ConfirmOptions): Promise<ConfirmResult> {
  mount();
  if (open.value) finish('close');
  opts.value = o;
  open.value = true;
  return new Promise((resolve) => (resolver = resolve));
}
