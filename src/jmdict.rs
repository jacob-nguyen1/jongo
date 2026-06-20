
pub fn lookup(word: &str, pos: crate::grammar::PartOfSpeech) -> Option<(String, Vec<String>)> {
    for entry in jmdict::entries() {
        // if the word matches kanji or reading
        if entry.kanji_elements().any(|k| k.text == word) || entry.reading_elements().any(|r| r.text == word) {
            // find senses that match the requested part of speech
            let matching_senses: Vec<_> = entry.senses().filter(|sense| {
                let jm_pos: Vec<_> = sense.parts_of_speech().collect();
                if jm_pos.is_empty() {
                    true // if no POS is explicitly defined for this sense, assume it inherits and might match
                } else {
                    jm_pos.iter().any(|p| pos.matches_jmdict(p))
                }
            }).collect();

            // if we found any matching senses, return them!
            if !matching_senses.is_empty() {
                let kana = entry.reading_elements().next()?.text.to_string();
                let glosses: Vec<String> = matching_senses.into_iter()
                    .flat_map(|s| s.glosses())
                    .filter(|g| g.language == jmdict::GlossLanguage::English)
                    .map(|g| g.text.to_string())
                    .collect();
                return Some((kana, glosses));
            }
        }
    }
    None
}


pub fn debug_word(word: &str) {
    // finds all jmdict data for a specific word
    let matches: Vec<_> = jmdict::entries().filter(|e| {
        e.kanji_elements().any(|k| k.text == word)
            || e.reading_elements().any(|r| r.text == word)
    }).collect();

    println!("=== {} === ({} entries found)", word, matches.len());

    for entry in matches {
        let kanji: Vec<_>   = entry.kanji_elements().map(|k| k.text).collect();
        let reading: Vec<_> = entry.reading_elements().map(|r| r.text).collect();
        println!("  kanji={:?} reading={:?}", kanji, reading);
        for (i, sense) in entry.senses().enumerate() {
            let pos: Vec<_>    = sense.parts_of_speech().map(|p| format!("{:?}", p)).collect();
            let glosses: Vec<_> = sense.glosses()
                .filter(|g| g.language == jmdict::GlossLanguage::English)
                .map(|g| g.text).collect();
            println!("    sense {}: {:?} → {:?}", i+1, pos, glosses);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_okaasan() {
        let word = "葉"; // 私は食べる
        let result = lookup(word, crate::grammar::PartOfSpeech::Noun).unwrap();
        println!("original word: {}\nkana: {}\nenglish: {:?}", word, result.0, result.1);
        // assert!(result.0.contains("おかあさん"));
        // assert!(result.1.contains(&"mother"));
    }

    #[test]
    fn debug() {
        debug_word("は");
        debug_word("に");
    }
}
