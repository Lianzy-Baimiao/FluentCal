// Made-up demo data with a fixed date, shared by readme-shots.mjs (README screenshots) and
// parity.mjs (0.2 vs native comparison), so neither ever shows real data.

// "Today" is Wednesday 2026-10-14: October shows the National Day 休/班 days and two solar terms.
export const NOW = new Date('2026-10-14T02:30:00Z'); // 10:30 in Asia/Shanghai
const day = (n) => new Date(Date.UTC(2026, 9, 14 + n)).toISOString().slice(0, 10);

export function demoDoc(settings) {
  const ts = NOW.getTime();
  const tasks = {};
  let order = 0;
  const add = (id, t) => (tasks[id] = { id, end: t.start, progress: 0, done: false, order: ++order, createdAt: ts, updatedAt: ts, ...t });
  const groups = {
    web: { id: 'web', name: '官网改版', color: 'blue', order: 1 },
    app: { id: 'app', name: '小程序 2.0', color: 'teal', order: 2 },
  };
  add('research', { title: '需求调研', start: day(-5), end: day(-3), groupId: 'web', progress: 100, done: true });
  add('design', { title: '视觉设计', start: day(-2), end: day(2), groupId: 'web', progress: 45, color: 'purple', deps: ['research'] });
  add('dev', { title: '前端开发', start: day(3), end: day(8), groupId: 'web', color: 'blue', deps: ['design'] });
  add('qa', { title: '测试验收', start: day(9), end: day(11), groupId: 'web', color: 'orange', deps: ['dev'] });
  add('launch', { title: '正式上线', start: day(12), groupId: 'web', milestone: true, color: 'red', deps: ['qa'] });
  add('proto', { title: '交互原型', start: day(-4), end: day(-1), groupId: 'app', progress: 100, done: true, color: 'teal' });
  add('api', { title: '接口联调', start: day(0), end: day(5), groupId: 'app', progress: 15, color: 'teal', deps: ['proto'] });
  add('beta', { title: '内测', start: day(6), end: day(9), groupId: 'app', color: 'indigo', deps: ['api'] });
  add('weekly', { title: '交周报', start: day(2), time: '17:00', reminders: [15], repeat: { freq: 'weekly', weekdays: [5] } });
  add('gym', { title: '健身', start: day(-8), color: 'green', repeat: { freq: 'weekly', weekdays: [2, 4] } });
  add('review', { title: '评审会', start: day(1), time: '14:00', color: 'indigo' });
  add('dentist', { title: '牙医复诊', start: day(7), time: '09:30', color: 'pink' });
  add('reading', { title: '读完《设计心理学》', start: day(14), end: day(18), color: 'yellow' });
  add('trip', { title: '国庆出游', start: day(-13), end: day(-10), color: 'green' });
  add('mom', { title: '妈妈生日', start: day(10), color: 'red', countdown: true, repeat: { freq: 'yearly' } });
  add('bill', { title: '交房租', start: day(17), color: 'gray', repeat: { freq: 'monthly' } });
  add('expense', { title: '报销差旅费', start: day(-1), done: true });
  const dayMarks = { [day(10)]: { color: 'red' }, [day(12)]: { color: 'orange' } };
  return {
    version: 1, tasks, groups, dayMarks, holidays: {},
    settings: { general: { seeded: true }, holidays: { autoUpdate: false, lastCheck: ts }, ...settings },
  };
}
