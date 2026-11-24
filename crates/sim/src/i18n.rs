use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Slovak,
    Czech,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::English, Language::Slovak, Language::Czech];

    /// Each language is named in itself, so it can be found whatever language is active.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Slovak => "Slovenčina",
            Language::Czech => "Čeština",
        }
    }
}

/// The interface language is global: every screen reads it, and only the settings change it.
static ACTIVE: AtomicU8 = AtomicU8::new(0);

pub fn set_language(language: Language) {
    ACTIVE.store(language as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    Language::ALL[ACTIVE.load(Ordering::Relaxed) as usize % Language::ALL.len()]
}

/// Every translation maps the English text to its own; placeholders `{}` keep their count and order.
const SLOVAK: &str = include_str!("../../../assets/i18n/sk.json");
const CZECH: &str = include_str!("../../../assets/i18n/cs.json");

struct Translations {
    slovak: HashMap<String, String>,
    czech: HashMap<String, String>,
}

fn parse(language: &str, json: &str) -> HashMap<String, String> {
    serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("the {language} translation is not valid JSON: {error}"))
}

fn translations() -> &'static Translations {
    static TABLE: OnceLock<Translations> = OnceLock::new();
    TABLE.get_or_init(|| Translations {
        slovak: parse("Slovak", SLOVAK),
        czech: parse("Czech", CZECH),
    })
}

fn lookup(english: &str) -> Option<&'static str> {
    let table = match language() {
        Language::English => return None,
        Language::Slovak => &translations().slovak,
        Language::Czech => &translations().czech,
    };
    table.get(english).map(String::as_str)
}

/// The interface text in the active language; texts without a translation stay in English.
pub fn tr(english: &'static str) -> &'static str {
    lookup(english).unwrap_or(english)
}

/// Like `tr`, for texts that are not compile-time constants (saved outcomes, enum names).
pub fn tr_owned(english: &str) -> String {
    lookup(english).map_or_else(|| english.to_string(), str::to_string)
}

/// Translates a template, then fills its `{}` placeholders in order.
pub fn tr_fmt(template: &'static str, values: &[&dyn Display]) -> String {
    let mut parts = tr(template).split("{}");
    let mut filled = parts.next().unwrap_or_default().to_string();
    for (part, value) in parts.zip(
        values
            .iter()
            .map(|v| v.to_string())
            .chain(std::iter::repeat_with(String::new)),
    ) {
        filled.push_str(&value);
        filled.push_str(part);
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_translation_keeps_its_placeholders() {
        let holes = |text: &str| text.matches("{}").count();
        let table = translations();
        for (name, language) in [("Slovak", &table.slovak), ("Czech", &table.czech)] {
            for (english, translated) in language {
                assert_eq!(holes(english), holes(translated), "{name}: {english}");
            }
        }
    }

    #[test]
    fn every_language_translates_the_same_texts() {
        let table = translations();
        let missing_in_czech: Vec<&String> = table
            .slovak
            .keys()
            .filter(|english| !table.czech.contains_key(*english))
            .collect();
        let missing_in_slovak: Vec<&String> = table
            .czech
            .keys()
            .filter(|english| !table.slovak.contains_key(*english))
            .collect();
        assert!(
            missing_in_czech.is_empty(),
            "no Czech for {missing_in_czech:?}"
        );
        assert!(
            missing_in_slovak.is_empty(),
            "no Slovak for {missing_in_slovak:?}"
        );
    }

    #[test]
    fn no_english_text_is_listed_twice() {
        for (name, json) in [("Slovak", SLOVAK), ("Czech", CZECH)] {
            let keys = json
                .lines()
                .filter(|line| line.trim_start().starts_with('"'))
                .count();
            assert_eq!(
                keys,
                parse(name, json).len(),
                "{name} lists an English text twice"
            );
        }
    }

    #[test]
    fn templates_fill_in_order_and_unknown_texts_stay_english() {
        set_language(Language::English);
        assert_eq!(tr_fmt("POWER {} / {}", &[&40, &120]), "POWER 40 / 120");
        assert_eq!(tr("No such text"), "No such text");
    }
}
