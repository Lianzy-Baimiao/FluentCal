import { ref } from 'vue';
import { today } from '@/lib/date';

/** Today's date; updates shortly after midnight (and after sleep/resume). */
export const now = ref(today());

let timer: ReturnType<typeof setInterval> | undefined;

export function startClock(onDayChange?: (day: string, prev: string) => void): void {
  if (timer) return;
  const tick = () => {
    const t = today();
    if (t !== now.value) {
      const prev = now.value;
      now.value = t;
      onDayChange?.(t, prev);
    }
  };
  timer = setInterval(tick, 30_000);
  document.addEventListener('visibilitychange', tick);
}
