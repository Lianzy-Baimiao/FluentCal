import { ref } from 'vue';
import en from '../locales/en-US.json';
import tw from '../locales/zh-TW.json';

export type Language = 'zh-CN' | 'zh-TW' | 'en-US';
export type LanguagePreference = 'system' | Language;
const KEY = 'fluentcal.web.language';

export function resolveLanguage(preference: string, system: string): Language {
  const parts = (preference === 'system' || !preference ? system : preference).replaceAll('_', '-').toLowerCase().split('-');
  if (parts[0] !== 'zh') return 'en-US';
  if (parts.includes('hans')) return 'zh-CN';
  return parts.some(p => ['hant', 'tw', 'hk', 'mo'].includes(p)) ? 'zh-TW' : 'zh-CN';
}

export function validPreference(value: string): value is LanguagePreference {
  return ['system', 'zh-CN', 'zh-TW', 'en-US'].includes(value);
}

function initialPreference(): LanguagePreference {
  try { const value = localStorage.getItem(KEY) ?? 'system'; return validPreference(value) ? value : 'system'; }
  catch { return 'system'; }
}

export const languagePreference = ref<LanguagePreference>(initialPreference());
// Legacy development views keep their original language; the shipped web viewer follows
// this browser's language, independently of the desktop that serves the data.
const web = typeof location !== 'undefined' && new URLSearchParams(location.search).get('w') === 'web';
export const language = ref<Language>(web ? resolveLanguage(languagePreference.value, navigator.language) : 'zh-CN');

export function formatMessage(template: string, args: unknown[]): string {
  return template.replace(/\{(\d+)\}/g, (match, n: string) => Number(n) < args.length ? String(args[Number(n)]) : match);
}

export function t(source: string, ...args: unknown[]): string {
  const catalog: Record<string, string> = language.value === 'en-US' ? en : language.value === 'zh-TW' ? tw : {};
  return formatMessage(catalog[source] ?? source, args);
}

export function setWebLanguage(value: string) {
  if (!validPreference(value)) return;
  languagePreference.value = value;
  language.value = resolveLanguage(value, navigator.language);
  try { localStorage.setItem(KEY, value); } catch { /* Storage may be blocked; selection still works for this page. */ }
  document.documentElement.lang = language.value;
}

if (web) {
  document.documentElement.lang = language.value;
  window.addEventListener('languagechange', () => {
    if (languagePreference.value === 'system') setWebLanguage('system');
  });
}
