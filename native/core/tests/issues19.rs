use fluentcal_core::{color::{hex_to_hsv, hsv_to_hex}, countdown::selected_countdowns, groups::summaries, model::Task, tasks::Doc};
use serde_json::json;

fn task(id: &str, group: Option<&str>, marked: bool) -> Task {
    Task { id:id.into(), title:id.into(), start:"2026-11-25".into(), end:"2026-11-25".into(), group_id:group.map(str::to_owned), countdown:Some(marked), ..Default::default() }
}

#[test]
fn countdown_sources_are_a_union_and_empty_means_none() {
    let tasks=vec![task("a",Some("birthday"),true), task("b",Some("birthday"),false), task("c",None,true), task("d",None,false)];
    let ids=|marked,groups:&[String]| selected_countdowns(&tasks,"2026-11-21",false,false,false,marked,groups).into_iter().map(|x|x.key).collect::<Vec<_>>();
    assert_eq!(ids(true,&[]),["a","c"]);
    assert_eq!(ids(false,&["birthday".into()]),["a","b"]);
    assert_eq!(ids(true,&["birthday".into()]),["a","b","c"]);
    assert!(ids(false,&[]).is_empty());
    assert_eq!(ids(false,&["".into()]),["c","d"]);
}

#[test]
fn countdown_defaults_match_issue18() {
    let settings = fluentcal_core::settings::Settings::default();
    assert!(settings.calendar.countdown_marked);
    assert!(settings.calendar.countdown_festival);
    assert!(!settings.calendar.holiday_countdown);
    assert!(!settings.calendar.countdown_term);
    let loaded = fluentcal_core::settings::Settings::from_value(&serde_json::Value::Null);
    assert!(loaded.calendar.countdown_marked);
    assert!(loaded.calendar.countdown_festival);
    assert!(!loaded.calendar.holiday_countdown);
    assert!(!loaded.calendar.countdown_term);
    let explicit_legacy = fluentcal_core::settings::Settings::from_value(&json!({
        "calendar": { "countdownMarked": true, "countdownFestival": false, "holidayCountdown": true }
    }));
    assert!(explicit_legacy.calendar.countdown_marked);
    assert!(!explicit_legacy.calendar.countdown_festival);
    assert!(explicit_legacy.calendar.holiday_countdown);
}

#[test]
fn multi_day_statutory_holiday_countdown_uses_only_the_first_day() {
    use fluentcal_core::{countdown::selected_countdowns, lunar::set_holiday_overrides, model::{HolidayDay, HolidayYear}};
    use std::collections::BTreeMap;

    set_holiday_overrides(BTreeMap::from([("2027".into(), HolidayYear {
        year: 2027,
        fetched_at: 0,
        days: ["2027-01-01", "2027-01-02", "2027-01-03"].into_iter().map(|date| HolidayDay {
            date: date.into(), name: "元旦".into(), is_off_day: true,
        }).collect(),
    })]));
    let items = selected_countdowns(&[], "2026-11-22", true, true, false, false, &[]);
    let new_year: Vec<_> = items.iter().filter(|item| item.date.starts_with("2027-01-") && item.label.contains("元旦")).collect();
    assert_eq!(new_year.len(), 1, "the New Year holiday is one countdown, not one per day: {new_year:?}");
    assert_eq!(new_year[0].date, "2027-01-01");
    assert_eq!(new_year[0].days, 40);
    set_holiday_overrides(BTreeMap::from([("2026".into(), HolidayYear::default())]));
}

#[test]
fn group_counts_include_done_and_do_not_expand_recurrences() {
    let mut doc=Doc::from_value(&json!({"groups":{"g":{"id":"g","name":"工作","color":"blue","order":1}}}));
    let a=task("a",Some("g"),false);
    let mut b=task("b",Some("g"),false); b.done=true;
    let c=task("c",Some("deleted"),false);
    let d=task("d",None,false);
    doc.tasks=vec![a,b,c,d].into_iter().map(|t|(t.id.clone(),t)).collect();
    let rows=summaries(&doc);
    assert_eq!(rows.iter().map(|g|(g.id.as_str(),g.count)).collect::<Vec<_>>(),[("",2),("g",2)]);
    assert_eq!(rows[1].color.as_deref(),Some("blue"));
}

#[test]
fn countdown_festival_and_term_sources_are_independent_and_skip_today() {
    use fluentcal_core::countdown::{countdowns, next_observance};
    assert!(next_observance("2026-11-03", false).is_some_and(|(_, date)| date.as_str() > "2026-11-03"));
    assert!(next_observance("2026-11-03", true).is_some_and(|(_, date)| date.as_str() > "2026-11-03"));
    assert_eq!(next_observance("2026-04-04", true), Some(("清明".into(), "2026-04-05".into())));
    assert!(countdowns(&[], "2026-04-04", false).is_empty());
    assert_eq!(countdowns(&[], "2026-09-20", true).iter().map(|item| item.date.as_str()).collect::<Vec<_>>(), ["2026-09-25"]);
}

#[test]
fn hsv_matches_known_rgb_colors_and_round_trips() {
    for (h,expected) in [(0.0,"#FF0000"),(60.0,"#FFFF00"),(120.0,"#00FF00"),(180.0,"#00FFFF"),(240.0,"#0000FF"),(300.0,"#FF00FF")] {
        assert_eq!(hsv_to_hex(h,100.0,100.0),expected);
    }
    assert_eq!(hsv_to_hex(120.0,0.0,50.0),"#808080");
    assert_eq!(hsv_to_hex(300.0,100.0,0.0),"#000000");
    for hex in ["#0078D4","#B5470D","#FFFFFF","#000000","#7A7574"] { let(h,s,v)=hex_to_hsv(hex); assert_eq!(hsv_to_hex(h,s,v),hex); }
}
