//! First-run sample data (0.2's src/lib/seed.ts): a small project for the Gantt view and a
//! weekly task, all marked `sample` so "清除示例数据" can remove them.

use crate::i18n::{tr};

use crate::date::{add_days, weekday};
use crate::model::{Freq, Group, RepeatRule, Task};
use crate::tasks::uid;

pub fn sample_data(now: &str, ts: i64) -> (Vec<Group>, Vec<Task>) {
    let g = Group { id: uid(), name: tr("示例项目：官网改版").into(), color: "blue".into(), order: 1.0, sample: Some(true), ..Default::default() };
    let base = |title: &str, start: String, end: Option<String>, order: f64| Task {
        id: uid(),
        title: title.into(),
        end: end.unwrap_or_else(|| start.clone()),
        start,
        order,
        sample: Some(true),
        created_at: ts,
        updated_at: ts,
        ..Default::default()
    };
    let mut research = base(tr("需求调研"), add_days(now, -4), Some(add_days(now, -1)), 1.0);
    research.group_id = Some(g.id.clone());
    research.progress = 100.0;
    research.done = true;
    let mut design = base(tr("视觉设计"), add_days(now, 0), Some(add_days(now, 5)), 2.0);
    design.group_id = Some(g.id.clone());
    design.progress = 40.0;
    design.color = Some("purple".into());
    design.deps = Some(vec![research.id.clone()]);
    let mut dev = base(tr("前端开发"), add_days(now, 6), Some(add_days(now, 16)), 3.0);
    dev.group_id = Some(g.id.clone());
    dev.color = Some("teal".into());
    dev.deps = Some(vec![design.id.clone()]);
    let mut test = base(tr("测试验收"), add_days(now, 17), Some(add_days(now, 21)), 4.0);
    test.group_id = Some(g.id.clone());
    test.color = Some("orange".into());
    test.deps = Some(vec![dev.id.clone()]);
    let mut launch = base(tr("正式上线"), add_days(now, 22), None, 5.0);
    launch.group_id = Some(g.id.clone());
    launch.milestone = Some(true);
    launch.color = Some("red".into());
    launch.deps = Some(vec![test.id.clone()]);
    let friday = add_days(now, ((5 + 7 - weekday(now)) % 7) as i32);
    let mut report = base(tr("交周报"), friday, None, 6.0);
    report.time = Some("17:00".into());
    report.reminders = Some(vec![15]);
    report.repeat = Some(RepeatRule { freq: Some(Freq::Weekly), weekdays: Some(vec![5]), ..Default::default() });
    (vec![g], vec![research, design, dev, test, launch, report])
}
