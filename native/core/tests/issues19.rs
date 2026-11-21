use fluentcal_core::{color::{hex_to_hsv, hsv_to_hex}, countdown::selected_countdowns, groups::summaries, model::Task, tasks::Doc};
use serde_json::json;

fn task(id: &str, group: Option<&str>, marked: bool) -> Task {
    Task { id:id.into(), title:id.into(), start:"2026-11-25".into(), end:"2026-11-25".into(), group_id:group.map(str::to_owned), countdown:Some(marked), ..Default::default() }
}

#[test]
fn countdown_sources_are_a_union_and_empty_means_none() {
    let tasks=vec![task("a",Some("birthday"),true), task("b",Some("birthday"),false), task("c",None,true), task("d",None,false)];
    let ids=|marked,groups:&[String]| selected_countdowns(&tasks,"2026-11-21",false,marked,groups).into_iter().map(|x|x.key).collect::<Vec<_>>();
    assert_eq!(ids(true,&[]),["a","c"]);
    assert_eq!(ids(false,&["birthday".into()]),["a","b"]);
    assert_eq!(ids(true,&["birthday".into()]),["a","b","c"]);
    assert!(ids(false,&[]).is_empty());
    assert_eq!(ids(false,&["".into()]),["c","d"]);
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
fn hsv_matches_known_rgb_colors_and_round_trips() {
    for (h,expected) in [(0.0,"#FF0000"),(60.0,"#FFFF00"),(120.0,"#00FF00"),(180.0,"#00FFFF"),(240.0,"#0000FF"),(300.0,"#FF00FF")] {
        assert_eq!(hsv_to_hex(h,100.0,100.0),expected);
    }
    assert_eq!(hsv_to_hex(120.0,0.0,50.0),"#808080");
    assert_eq!(hsv_to_hex(300.0,100.0,0.0),"#000000");
    for hex in ["#0078D4","#B5470D","#FFFFFF","#000000","#7A7574"] { let(h,s,v)=hex_to_hsv(hex); assert_eq!(hsv_to_hex(h,s,v),hex); }
}
