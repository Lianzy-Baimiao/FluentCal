import { reactive } from 'vue';
import { call, on, type SyncStatus } from '@/api';

/** Cloud sync status, shared by the settings page and the calendar's sync icon. */
export const syncStatus = reactive<SyncStatus>({
  enabled: false,
  provider: '',
  syncing: false,
  lastSync: null,
  error: null,
  errorKind: null,
  halted: false,
  pending: false,
  remoteDevice: null,
  remoteSavedAt: null,
});

let started = false;

/** Loads the status once and follows `sync://status`. */
export function watchSync(): void {
  if (started) return;
  started = true;
  on<SyncStatus>('sync://status', (s) => Object.assign(syncStatus, s));
  void call<SyncStatus>('sync_status').then((s) => s && Object.assign(syncStatus, s));
}

/** "Sync now"; the result arrives through the status as well. */
export async function syncNow(): Promise<void> {
  syncStatus.syncing = true;
  try {
    const s = await call<SyncStatus>('sync_now');
    if (s) Object.assign(syncStatus, s);
  } catch (e) {
    syncStatus.error = String(e);
  } finally {
    syncStatus.syncing = false;
  }
}
