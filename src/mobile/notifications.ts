import { Capacitor } from "@capacitor/core";
import { LocalNotifications } from "@capacitor/local-notifications";
import { computeReminders } from "@/lib/reminders";
import type { Doc } from "@/types";

let queue = Promise.resolve();
/** Rebuild all pending alarms, including deletion, completion and global disable. */
export function updateNotifications(doc: Doc): Promise<void> {
  if (!Capacitor.isNativePlatform()) return Promise.resolve();
  const snapshot: Doc = JSON.parse(JSON.stringify(doc));
  const job = queue.then(async () => {
    const pending = await LocalNotifications.getPending();
    if (pending.notifications.length) await LocalNotifications.cancel(pending);
    if (
      !snapshot.settings.reminders.enabled ||
      (await LocalNotifications.checkPermissions()).display !== "granted"
    )
      return;
    const items = computeReminders(
      Object.values(snapshot.tasks),
      Date.now(),
      snapshot.settings.reminders.allDayTime,
      30 * 86_400_000,
      0,
    ).slice(0, 64);
    if (!items.length) return;
    await LocalNotifications.schedule({
      notifications: items.map((x, i) => ({
        id: 1000 + i,
        title: x.title,
        body: x.body,
        extra: { taskId: x.taskId, date: x.date },
        schedule: { at: new Date(x.atMs), allowWhileIdle: true },
      })),
    });
  });
  queue = job.catch(() => {});
  return job;
}
