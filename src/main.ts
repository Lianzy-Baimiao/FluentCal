import './fluent/tokens.css';
import './fluent/base.css';
import { createApp, type Component } from 'vue';

// Each Tauri window loads the same page; the window label picks the root component.
// In a plain browser (dev, screenshots) use ?w=desk|settings|editor|gallery.
const label: string =
  (window as { __TAURI_INTERNALS__?: { metadata?: { currentWebview?: { label?: string } } } })
    .__TAURI_INTERNALS__?.metadata?.currentWebview?.label ??
  new URLSearchParams(location.search).get('w') ??
  'desk';

document.documentElement.dataset.window = label;
// No Mica or desktop behind the page in a plain browser: window-shell.css paints stand-ins.
if (!('__TAURI_INTERNALS__' in window)) document.documentElement.classList.add('browser');

const roots = import.meta.glob<{ default: Component }>('./windows/*/*App.vue');
const key = Object.keys(roots).find((k) => k.startsWith(`./windows/${label}/`)) ?? './windows/desk/DeskApp.vue';
roots[key]?.().then((m) => createApp(m.default).mount('#app'));
