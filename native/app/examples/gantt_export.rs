//! Exports the Gantt chart of a parity scenario (scripts/parity.mjs) to SVG and PNG, without a
//! window: `gantt_export <scene.json> <out-base>` writes <out-base>.svg and .png.

use fluentcal_app::desk::gantt::{export, model};
use fluentcal_core::date::local_of_ms;
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::tokens;
use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sc: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("scenario")).expect("json");
    let doc = Doc::from_value(&sc["doc"]);
    let (today, _) = local_of_ms(sc["now"].as_f64().unwrap_or(0.0) as i64, sc["offset"].as_f64().unwrap_or(480.0) as i32);
    let dark = doc.settings.theme.mode == "dark";
    let accent = if doc.settings.theme.accent == "system" { sc["systemAccent"].as_str().unwrap_or("#0078D4").to_owned() } else { doc.settings.theme.accent.clone() };
    let t = tokens(dark, &accent, None);
    let m = model::compute(&doc, &today, &t, false);
    let (svg, w, h) = export::svg(&doc, &m, &t, &today);
    std::fs::write(format!("{}.svg", args[2]), &svg).unwrap();
    let t0 = std::time::Instant::now();
    let png = export::png(&svg, w, h).expect("png");
    std::fs::write(format!("{}.png", args[2]), png).unwrap();
    println!("{w}x{h}, png in {} ms", t0.elapsed().as_millis());
}
