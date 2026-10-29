import { computed, reactive, watch, watchEffect } from 'vue';
import { call, currentWindow, on, windowLabel, type AppInfo } from '@/api';
import { doc } from '@/stores/doc';
import { applyTheme } from '@/fluent/theme';
import { alpha, mixHex } from '@/lib/color';

export const sys = reactive<{ info: AppInfo | null }>({ info: null });

export async function refreshSystem(): Promise<void> {
  try {
    sys.info = await call<AppInfo>('app_info');
  } catch (e) {
    console.warn('app_info failed', e);
  }
}

export const isDark = computed(() => {
  const m = doc.settings.theme.mode;
  if (m === 'dark') return true;
  if (m === 'light') return false;
  return sys.info?.dark ?? matchMedia('(prefers-color-scheme: dark)').matches;
});

export const accentColor = computed(() => {
  const a = doc.settings.theme.accent;
  return a && a !== 'system' ? a : (sys.info?.accent ?? '#0078D4');
});

/** Mica needs Windows 11 (build 22000+); older systems get a solid window background. */
export const hasMica = computed(() => (sys.info?.build ?? 22000) >= 22000);

let started = false;

/** Applies theme, accent and motion settings to this window and keeps them in sync. */
export function startThemeSync(): void {
  if (started) return;
  started = true;
  void refreshSystem();
  let t: ReturnType<typeof setTimeout> | undefined;
  on('system://theme', () => {
    clearTimeout(t);
    t = setTimeout(refreshSystem, 250);
  });
  watchEffect(() => {
    const useSystem = doc.settings.theme.accent === 'system';
    const palette = useSystem && sys.info?.accentPalette?.length === 7 ? sys.info.accentPalette : undefined;
    applyTheme(document.documentElement, { dark: isDark.value, accent: accentColor.value, palette });
    document.documentElement.classList.toggle('no-motion', doc.settings.theme.reduceMotion);
    document.documentElement.classList.toggle('no-mica', !hasMica.value);
  });
  if (windowLabel !== 'desk') watch(isDark, (d) => void currentWindow.setTheme(d), { immediate: true });
}

/** CSS variables for the desk calendar surface, derived from appearance settings. */
export const deskVars = computed<Record<string, string>>(() => {
  const s = doc.settings.desk;
  const dark = isDark.value;
  const base = s.bgColor === 'auto' ? (dark ? '#202020' : '#F3F3F3') : s.bgColor;
  const cell = mixHex(base, '#FFFFFF', dark ? 0.07 : 0.85);
  const shadowText =
    s.textShadow === 'on' || (s.textShadow === 'auto' && !s.acrylic && s.bgOpacity < 35 && s.cellOpacity < 60);
  const scale = s.fontScale / 100;
  const holiday = s.holidayTextColor === 'auto' ? (dark ? '#FF99A4' : '#C42B1C') : s.holidayTextColor;
  const rest = s.restBgColor === 'auto' ? (dark ? 'rgba(0,0,0,0.16)' : 'rgba(0,0,0,0.035)') : alpha(s.restBgColor, 0.22);
  // Border and shadow fade out with the background so a transparent calendar has no frame.
  const k = Math.min(1, s.bgOpacity / 60);
  return {
    '--desk-bg': alpha(base, s.bgOpacity / 100),
    '--desk-radius': s.acrylic ? '8px' : `${s.radius}px`,
    '--desk-pad': s.acrylic ? '0px' : s.shadow ? '10px' : '2px',
    '--desk-stroke': dark ? `rgba(255,255,255,${(0.1 * k).toFixed(3)})` : `rgba(0,0,0,${(0.08 * k).toFixed(3)})`,
    '--desk-shadow':
      s.shadow && !s.acrylic && k > 0.05
        ? `0 4px 14px rgba(0,0,0,${(0.22 * k).toFixed(3)}), 0 0 2px rgba(0,0,0,${(0.12 * k).toFixed(3)})`
        : 'none',
    '--panel-bg': alpha(base, Math.max(0.82, s.bgOpacity / 100)),
    '--cell-bg': alpha(cell, s.cellOpacity / 100),
    '--cell-gap': `${s.cellGap}px`,
    '--line-gap': `${s.lineSpacing}px`,
    '--fs': String(scale),
    '--today': s.todayColor === 'accent' ? 'var(--accent-fill)' : s.todayColor,
    '--rest-tint': rest,
    '--holiday-text': holiday,
    '--term-text': s.termTextColor === 'auto' ? 'var(--accent-text)' : s.termTextColor,
    '--text-glow': shadowText ? (dark ? '0 0 3px rgba(0,0,0,.85), 0 1px 2px rgba(0,0,0,.7)' : '0 0 3px rgba(255,255,255,.9), 0 1px 2px rgba(255,255,255,.8)') : 'none',
  };
});
