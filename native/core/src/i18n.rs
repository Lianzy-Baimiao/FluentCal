//! Local UI language. Catalog keys are source messages, never task/user data or protocol IDs.
//! The desktop has one UI language; background notifications use the same selection.

use std::collections::BTreeMap;
use std::sync::{atomic::{AtomicU8, Ordering}, LazyLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Language { SimplifiedChinese, TraditionalChinese, English }

impl Language {
    pub fn tag(self) -> &'static str {
        match self { Self::SimplifiedChinese => "zh-CN", Self::TraditionalChinese => "zh-TW", Self::English => "en-US" }
    }
}

/// Windows/browser BCP-47 tags. Explicit scripts take precedence over region aliases.
pub fn resolve(preference: &str, system: &str) -> Language {
    let tag = if preference == "system" || preference.is_empty() { system } else { preference };
    let tag = tag.replace('_', "-").to_ascii_lowercase();
    let parts: Vec<_> = tag.split('-').collect();
    if parts.first() == Some(&"zh") {
        if parts.contains(&"hans") { return Language::SimplifiedChinese; }
        if parts.contains(&"hant") || parts.iter().any(|p| matches!(*p, "tw" | "hk" | "mo")) { return Language::TraditionalChinese; }
        return Language::SimplifiedChinese;
    }
    Language::English
}

pub fn valid_preference(value: &str) -> bool { matches!(value, "system" | "zh-CN" | "zh-TW" | "en-US") }

static LANGUAGE: AtomicU8 = AtomicU8::new(Language::SimplifiedChinese as u8);
static EN: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| serde_json::from_str(include_str!("../../../locales/en-US.json")).expect("English catalog"));
static ZH_TW: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| serde_json::from_str(include_str!("../../../locales/zh-TW.json")).expect("Traditional Chinese catalog"));

pub fn current() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) { 1 => Language::TraditionalChinese, 2 => Language::English, _ => Language::SimplifiedChinese }
}

/// Returns whether the selected language changed.
pub fn set_language(language: Language) -> bool { LANGUAGE.swap(language as u8, Ordering::Relaxed) != language as u8 }

pub fn translate(language: Language, source: &str) -> &str {
    let catalog = match language {
        Language::SimplifiedChinese => return source,
        Language::TraditionalChinese => &*ZH_TW,
        Language::English => &*EN,
    };
    catalog.get(source).map(String::as_str).unwrap_or(source)
}

pub fn tr(source: &str) -> &str { translate(current(), source) }

/// Substitute numbered arguments once. Braces in user-provided arguments stay literal.
pub fn format_message(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(end) = rest.find('}') {
            if let Ok(index) = rest[1..end].parse::<usize>() {
                if let Some(arg) = args.get(index) { out.push_str(arg); rest = &rest[end + 1..]; continue; }
            }
        }
        out.push('{');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

pub fn trf(source: &str, args: &[String]) -> String { format_message(tr(source), args) }
