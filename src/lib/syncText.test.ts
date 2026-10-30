import { describe, expect, it } from 'vitest';
import type { SyncStatus } from '@/api';
import { ago, statusLine, uploadedBy } from './syncText';

const now = Date.UTC(2026, 9, 30, 4, 0);
const base: SyncStatus = { enabled: true, provider: 'WebDAV', syncing: false, lastSync: now - 5 * 60_000, error: null, errorKind: null, halted: false, pending: false, remoteDevice: null, remoteSavedAt: null };

describe('sync texts', () => {
  it('formats elapsed time', () => {
    expect(ago(now - 10_000, now)).toBe('刚刚');
    expect(ago(now - 5 * 60_000, now)).toBe('5 分钟前');
    expect(ago(now - 3 * 3_600_000, now)).toBe('3 小时前');
    expect(ago(now + 5_000, now)).toBe('刚刚'); // clock skew between PCs
  });

  it('describes the status', () => {
    expect(statusLine({ ...base, enabled: false }, now)).toBe('未开启');
    expect(statusLine(base, now)).toBe('已同步到 WebDAV，上次同步 5 分钟前');
    expect(statusLine({ ...base, pending: true }, now)).toContain('有修改等待上传');
    expect(statusLine({ ...base, syncing: true }, now)).toBe('正在同步到 WebDAV…');
    expect(statusLine({ ...base, error: '网络错误' }, now)).toBe('同步失败：网络错误（稍后自动重试）');
    expect(statusLine({ ...base, error: '密码不对', halted: true }, now)).toBe('已暂停：密码不对');
    expect(statusLine({ ...base, lastSync: null }, now)).toContain('还没有同步过');
  });

  it('names who uploaded last', () => {
    expect(uploadedBy('OFFICE-PC', now - 3_600_000, now)).toBe('最后由 OFFICE-PC 于 1 小时前上传');
    expect(uploadedBy(null, null, now)).toBe('');
  });
});
