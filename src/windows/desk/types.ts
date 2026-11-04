import type { ISODate } from '@/types';

/** Everything a day cell renders, precomputed by the month view. */
export interface CellInfo {
  date: ISODate;
  label: string;
  lunar: string;
  lunarKind: 'lunar' | 'month' | 'term' | 'festival';
  badge?: string;
  rest: boolean;
  today: boolean;
  dim: boolean;
  mark?: string;
  tip: string;
  aria: string;
}
