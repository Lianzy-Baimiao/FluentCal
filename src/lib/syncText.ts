import type { SyncStatus } from '@/api';

/** "刚刚", "5 分钟前", "3 小时前", else a date. */
export function ago(ms: number, now = Date.now()): string {
  const d = Math.max(0, now - ms);
  if (d < 60_000) return '刚刚';
  if (d < 3_600_000) return `${Math.floor(d / 60_000)} 分钟前`;
  if (d < 86_400_000) return `${Math.floor(d / 3_600_000)} 小时前`;
  return new Date(ms).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}

/** One line for the status card and the calendar's sync icon. */
export function statusLine(s: SyncStatus, now = Date.now()): string {
  if (!s.enabled) return '未开启';
  if (s.syncing) return `正在同步到 ${s.provider}…`;
  if (s.error) return s.halted ? `已暂停：${s.error}` : `同步失败：${s.error}（稍后自动重试）`;
  const last = s.lastSync ? `上次同步 ${ago(s.lastSync, now)}` : '还没有同步过';
  const pending = s.pending ? '，有修改等待上传' : '';
  return `已同步到 ${s.provider}，${last}${pending}`;
}

/** "最后由 OFFICE-PC 于 3 小时前上传" */
export function uploadedBy(device: string | null, at: number | null, now = Date.now()): string {
  if (!device && !at) return '';
  return `最后由 ${device ?? '另一台电脑'}${at ? ` 于 ${ago(at, now)}` : ''}上传`;
}
