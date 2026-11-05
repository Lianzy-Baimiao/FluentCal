import { beforeEach, expect, it, vi } from 'vitest';
import { emptyDocument } from './data';
const plugin = vi.hoisted(() => ({
  getPending: vi.fn(), cancel: vi.fn(), checkPermissions: vi.fn(), schedule: vi.fn(),
}));
vi.mock('@capacitor/core', () => ({ Capacitor: { isNativePlatform: () => true } }));
vi.mock('@capacitor/local-notifications', () => ({ LocalNotifications: plugin }));
beforeEach(() => {
  vi.resetModules(); vi.clearAllMocks();
  plugin.getPending.mockResolvedValue({ notifications: [{ id: 1000 }, { id: 1001 }] });
  plugin.checkPermissions.mockResolvedValue({ display: 'granted' });
  plugin.cancel.mockResolvedValue(undefined); plugin.schedule.mockResolvedValue(undefined);
});
it('cancels old reminders even after the last task is deleted', async () => {
  const { updateNotifications } = await import('./notifications');
  await updateNotifications(emptyDocument());
  expect(plugin.cancel).toHaveBeenCalledWith({ notifications: [{ id: 1000 }, { id: 1001 }] });
  expect(plugin.schedule).not.toHaveBeenCalled();
});
it('global disable clears pending alarms without requesting permission', async () => {
  const doc = emptyDocument(); doc.settings.reminders.enabled = false;
  const { updateNotifications } = await import('./notifications');
  await updateNotifications(doc);
  expect(plugin.cancel).toHaveBeenCalledOnce(); expect(plugin.checkPermissions).not.toHaveBeenCalled();
});
it('one failed native update does not poison later updates', async () => {
  plugin.getPending.mockRejectedValueOnce(new Error('native unavailable'));
  const { updateNotifications } = await import('./notifications');
  await expect(updateNotifications(emptyDocument())).rejects.toThrow();
  await expect(updateNotifications(emptyDocument())).resolves.toBeUndefined();
  expect(plugin.cancel).toHaveBeenCalledOnce();
});
