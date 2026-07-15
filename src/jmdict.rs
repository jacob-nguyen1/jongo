use crate::jmnedict::{self, ProperNounType};
use crate::labels::PartOfSpeech;

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

/// Query JMdict (common vocabulary) and optionally JMnedict (proper nouns).
pub fn lookup(
    word: &str,
    pos: PartOfSpeech,
    is_proper_noun: bool,
) -> Vec<LookupResult> {
    let mut results = lookup_jmdict(word, pos);

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
    pos: PartOfSpeech,
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
    pos: PartOfSpeech,
    is_proper_noun: bool,
) -> Option<LookupResult> {
    lookup(word, pos, is_proper_noun).into_iter().next()
}

fn lookup_jmdict(word: &str, pos: PartOfSpeech) -> Vec<LookupResult> {
    for entry in jmdict::entries() {
        let matches_form = entry.kanji_elements().any(|k| k.text == word)
            || entry.reading_elements().any(|r| r.text == word);

        if !matches_form {
            continue;
        }

        let matching_senses: Vec<_> = entry
            .senses()
            .filter(|sense| {
                let jm_pos: Vec<_> = sense.parts_of_speech().collect();
                jm_pos.is_empty() || jm_pos.iter().any(|p| pos.matches_jmdict(p))
            })
            .collect();

        if matching_senses.is_empty() {
            continue;
        }

        let Some(kana) = entry.reading_elements().next().map(|r| r.text.to_string()) else {
            continue;
        };

        let glosses: Vec<String> = matching_senses
            .into_iter()
            .flat_map(|s| s.glosses())
            .filter(|g| g.language == jmdict::GlossLanguage::English)
            .map(|g| g.text.to_string())
            .collect();

        if glosses.is_empty() {
            continue;
        }

        return vec![LookupResult {
            kana,
            glosses,
            source: DictSource::JMdict,
            noun_type: ProperNounType::NotApplicable,
        }];
    }

    Vec::new()
}

pub fn debug_word(word: &str) {
    let matches: Vec<_> = jmdict::entries()
        .filter(|e| {
            e.kanji_elements().any(|k| k.text == word)
                || e.reading_elements().any(|r| r.text == word)
        })
        .collect();

    println!("=== {} === ({} entries found)", word, matches.len());

    for entry in matches {
        let kanji: Vec<_> = entry.kanji_elements().map(|k| k.text).collect();
        let reading: Vec<_> = entry.reading_elements().map(|r| r.text).collect();
        println!("  kanji={:?} reading={:?}", kanji, reading);
        for (i, sense) in entry.senses().enumerate() {
            let pos: Vec<_> = sense.parts_of_speech().map(|p| format!("{:?}", p)).collect();
            let glosses: Vec<_> = sense
                .glosses()
                .filter(|g| g.language == jmdict::GlossLanguage::English)
                .map(|g| g.text)
                .collect();
            println!("    sense {}: {:?} → {:?}", i + 1, pos, glosses);
        }
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
        // 東京 exists in both dictionaries; JMdict wins when it matches.
        let result = lookup_first_result("東京", PartOfSpeech::Noun, true)
            .expect("expected a result for 東京");
        assert!(!result.glosses.is_empty());
    }

    #[test]
    fn test_jmnedict_fallback_when_jmdict_misses() {
        let word = "網走";
        if lookup_jmdict(word, PartOfSpeech::Noun).is_empty() {
            let result = lookup_first_result(word, PartOfSpeech::Noun, false)
                .expect("expected JMnedict fallback for 網走");
            assert_eq!(result.source, DictSource::JMnedict);
        }
    }

    #[test]
    fn debug() {
        debug_word("は");
        debug_word("に");
    }
}
