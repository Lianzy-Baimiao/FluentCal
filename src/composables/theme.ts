import { computed, reactive, watch, watchEffect } from 'vue';
import { call, currentWindow, on, windowLabel, isMobile, type AppInfo } from '@/api';
import { doc } from '@/stores/doc';
import { applyTheme, bestMonochromeText, bestTextOn, contrast, deriveAccent } from '@/fluent/theme';
import { alpha, mixHex } from '@/lib/color';
import { colorOpacity, compositeColor, opaqueColor, setColorOpacity } from '@/lib/rgba';

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
    applyTheme(document.documentElement, {
      dark: isDark.value,
      accent: isMobile ? '#0078D4' : accentColor.value,
      palette: isMobile ? undefined : palette,
    });
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
  const opacity = colorOpacity(base);
  const underlay = dark ? '#202020' : '#F3F3F3';
  const deskSurface = compositeColor(alpha(base, s.bgOpacity / 100), underlay);
  const cellSurface = compositeColor(alpha(cell, s.cellOpacity * opacity / 100), deskSurface);
  const calendarText = s.textColor === 'auto' ? bestMonochromeText(cellSurface) : opaqueColor(s.textColor);
  const outlineColor = contrast(calendarText, '#FFFFFF') >= contrast(calendarText, '#000000') ? '#FFFFFF' : '#000000';
  const palette = doc.settings.theme.accent === 'system' ? sys.info?.accentPalette : undefined;
  const accent = palette?.length === 7 ? palette[dark ? 1 : 4] : deriveAccent(accentColor.value)[dark ? 1 : 4];
  const todayBase = s.todayColor === 'accent' ? setColorOpacity(accent, palette?.length === 7 ? 1 : colorOpacity(accentColor.value)) : s.todayColor;
  const today = setColorOpacity(todayBase, colorOpacity(todayBase) * Math.max(0, Math.min(100, s.todayOpacity)) / 100);
  const scale = s.fontScale / 100;
  const holiday = s.holidayTextColor === 'auto' ? (dark ? '#FF99A4' : '#C42B1C') : s.holidayTextColor;
  const rest = s.restBgColor === 'auto' ? (dark ? 'rgba(0,0,0,0.16)' : 'rgba(0,0,0,0.035)') : alpha(s.restBgColor, 0.22);
  const outlineOn = s.textOutlineWidth > 0 && s.textShadow !== 'off' && (s.textShadow === 'on' || (!s.acrylic && s.bgOpacity * opacity < 35 && s.cellOpacity * opacity < 60));
  const outline = outlineOn ? Array.from({ length: 8 }, (_, i) => {
    const offsets = [[-1, 0], [1, 0], [0, -1], [0, 1], [-0.7, -0.7], [0.7, -0.7], [-0.7, 0.7], [0.7, 0.7]] as const;
    const [x, y] = offsets[i];
    return `${(x * s.textOutlineWidth).toFixed(1)}px ${(y * s.textOutlineWidth).toFixed(1)}px 0 ${outlineColor}`;
  }).join(', ') : 'none';
  // Border and shadow fade out with the background so a transparent calendar has no frame.
  const k = Math.min(1, s.bgOpacity * opacity / 60);
  return {
    '--desk-bg': alpha(base, s.bgOpacity / 100),
    '--desk-radius': s.acrylic ? '8px' : `${s.radius}px`,
    '--desk-pad': s.acrylic ? '0px' : s.shadow ? '10px' : '2px',
    '--calendar-font-family': ({
      auto: 'var(--font-ui)',
      'segoe-variable': '"Segoe UI Variable Text", "Microsoft YaHei UI", sans-serif',
      segoe: '"Segoe UI", "Microsoft YaHei UI", sans-serif',
      'yahei-ui': '"Microsoft YaHei UI", "Segoe UI", sans-serif',
      yahei: '"Microsoft YaHei", "Segoe UI", sans-serif',
      arial: 'Arial, "Microsoft YaHei UI", sans-serif',
    } as const)[s.fontFamily],
    '--calendar-text': calendarText,
    '--calendar-text-secondary': alpha(calendarText, 0.82),
    '--calendar-text-tertiary': alpha(calendarText, 0.66),
    '--calendar-outline': outline,
    '--desk-stroke': dark ? `rgba(255,255,255,${(0.1 * k).toFixed(3)})` : `rgba(0,0,0,${(0.08 * k).toFixed(3)})`,
    '--desk-shadow':
      s.shadow && !s.acrylic && k > 0.05
        ? `0 4px 14px rgba(0,0,0,${(0.22 * k).toFixed(3)}), 0 0 2px rgba(0,0,0,${(0.12 * k).toFixed(3)})`
        : 'none',
    '--panel-bg': alpha(opaqueColor(base), Math.max(0.82, s.bgOpacity * opacity / 100)),
    '--cell-bg': alpha(cell, s.cellOpacity * opacity / 100),
    '--cell-gap': `${s.cellGap}px`,
    '--line-gap': `${s.lineSpacing}px`,
    '--fs': String(scale),
    '--today': today,
    '--today-text': s.todayStyle === 'fill' ? bestTextOn(today, cell) : 'var(--text-primary)',
    '--today-style': s.todayStyle,
    '--rest-tint': rest,
    '--holiday-text': holiday,
    '--term-text': s.termTextColor === 'auto' ? 'var(--accent-text)' : s.termTextColor,
    '--text-glow': outline,
  };
});
