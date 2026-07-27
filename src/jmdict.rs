use crate::jmnedict::{self, ProperNounType};
use std::sync::LazyLock;

/// The source dictionary a result came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictSource {
    JMdict,
    JMnedict,
}

/// A single lookup result, preserving provenance metadata.
#[derive(Debug, Clone)]
pub struct LookupResult {
    pub kana: String,
    pub glosses: Vec<String>,
    pub source: DictSource,
    pub noun_type: ProperNounType,
}

const JMDICT_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/jmdict.bin"));

#[derive(Debug, serde::Deserialize)]
struct JmdictData<'a> {
    #[serde(borrow)]
    entries: Vec<JmdictEntry<'a>>,
    #[serde(borrow)]
    by_form: Vec<(&'a str, Vec<u32>)>,
}

#[derive(Debug, serde::Deserialize)]
struct JmdictEntry<'a> {
    kana: &'a str,
    #[serde(borrow)]
    glosses: Vec<&'a str>,
}

static JMDICT_INDEX: LazyLock<JmdictData<'static>> = LazyLock::new(|| {
    postcard::from_bytes(JMDICT_BIN).expect("failed to decode jmdict.bin")
});

/// Query JMdict (common vocabulary) and optionally JMnedict (proper nouns).
pub fn lookup(
    word: &str,
    _pos: crate::labels::PartOfSpeech,
    is_proper_noun: bool,
) -> Vec<LookupResult> {
    let mut results = lookup_jmdict(word);

    let should_check_names = results.is_empty() || is_proper_noun;
    if should_check_names {
        for hit in jmnedict::lookup_name(word) {
            results.push(LookupResult {
                kana: hit.kana,
                glosses: hit.glosses,
                source: DictSource::JMnedict,
                noun_type: hit.noun_type,
            });
        }
    }

    results
}

/// Returns only the first (highest-priority) result.
pub fn lookup_first(
    word: &str,
    pos: crate::labels::PartOfSpeech,
    is_proper_noun: bool,
) -> Option<(String, Vec<String>)> {
    lookup(word, pos, is_proper_noun)
        .into_iter()
        .next()
        .map(|r| (r.kana, r.glosses))
}

/// Returns the first result with full provenance metadata.
pub fn lookup_first_result(
    word: &str,
    pos: crate::labels::PartOfSpeech,
    is_proper_noun: bool,
) -> Option<LookupResult> {
    lookup(word, pos, is_proper_noun).into_iter().next()
}

pub fn lookup_all_glosses(word: &str, pos: crate::labels::PartOfSpeech, is_proper_noun: bool) -> Vec<String> {
    lookup(word, pos, is_proper_noun)
        .into_iter()
        .flat_map(|r| r.glosses)
        .collect()
}

/// Check whether a base form is present in the JMdict vocabulary.
pub fn is_native_verb(word: &str) -> bool {
    !lookup_jmdict(word).is_empty()
}

fn lookup_jmdict(word: &str) -> Vec<LookupResult> {
    let mut results = Vec::new();
    if let Ok(idx) = JMDICT_INDEX.by_form.binary_search_by_key(&word, |&(f, _)| f) {
        for &entry_id in &JMDICT_INDEX.by_form[idx].1 {
            if let Some(entry) = JMDICT_INDEX.entries.get(entry_id as usize) {
                results.push(LookupResult {
                    kana: entry.kana.to_string(),
                    glosses: entry.glosses.iter().map(|&s| s.to_string()).collect(),
                    source: DictSource::JMdict,
                    noun_type: ProperNounType::NotApplicable,
                });
            }
        }
    }
    results
}

pub fn debug_word(word: &str) {
    let matches = lookup_jmdict(word);
    println!("=== {} === ({} entries found)", word, matches.len());
    for (i, entry) in matches.into_iter().enumerate() {
        println!("  {}. kana={} glosses={:?}", i + 1, entry.kana, entry.glosses);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::PartOfSpeech;

    #[test]
    fn test_leaf_noun() {
        let word = "葉";
        let result = lookup_first(word, PartOfSpeech::Noun, false).unwrap();
        println!(
            "original word: {}\nkana: {}\nenglish: {:?}",
            word, result.0, result.1
        );
    }

    #[test]
    fn test_proper_noun_dual_lookup() {
        let result = lookup_first_result("東京", PartOfSpeech::Noun, true)
            .expect("expected a result for 東京");
        assert!(!result.glosses.is_empty());
    }

    #[test]
    fn test_jmnedict_fallback_when_jmdict_misses() {
        let word = "網走";
        if lookup_jmdict(word).is_empty() {
            let result = lookup_first_result(word, PartOfSpeech::Noun, false)
                .expect("expected JMnedict fallback for 網走");
            assert_eq!(result.source, DictSource::JMnedict);
        }
    }

    #[test]
    fn debug() {
        debug_word("は");
        debug_word("に");
        debug_word("串焼き");
    }
}
