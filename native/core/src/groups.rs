//! Group names, colours and task counts shared by pickers, filters and management.
use crate::{i18n::tr, tasks::Doc};
use std::collections::BTreeMap;

pub struct Summary {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub count: usize,
}

/// Ungrouped first, then real groups in their display order. Each task counts once,
/// including completed and recurring tasks; a missing group counts as ungrouped.
pub fn summaries(doc: &Doc) -> Vec<Summary> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for task in doc.tasks.values() {
        let id = task.group_id.as_deref().filter(|id| doc.groups.contains_key(*id)).unwrap_or("");
        *counts.entry(id).or_default() += 1;
    }
    let mut groups: Vec<_> = doc.groups.values().collect();
    groups.sort_by(|a, b| a.order.total_cmp(&b.order).then(a.id.cmp(&b.id)));
    let mut rows = vec![Summary { id: String::new(), name: tr("未分组").into(), color: None, count: *counts.get("").unwrap_or(&0) }];
    rows.extend(groups.into_iter().map(|g| Summary {
        id: g.id.clone(), name: g.name.clone(), color: Some(g.color.clone()), count: *counts.get(g.id.as_str()).unwrap_or(&0),
    }));
    rows
}
