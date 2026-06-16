use jmdict::{entries, GlossLanguage, PartOfSpeech, Enum};

pub fn lookup(word: &str) -> Option<(String, Vec<String>)> {
    // finds target string
    let entry = entries().find(|e|
         {e.kanji_elements().any(|k| k.text == word) || e.reading_elements().any(|r| r.text == word)})?;

    // get the kana reading
    let kana = entry.reading_elements().next()?.text.to_string();

    // get all English glosses (definitions)
    let glosses: Vec<String> = entry.senses().flat_map(|s| s.glosses()).filter(|g| g.language == GlossLanguage::English).map(|g| g.text.to_string()).collect();
    for sense in entry.senses() {
        for pos in sense.parts_of_speech() {
            println!("{}", pos);
        }
    }
    Some((kana, glosses)) 
}

pub fn debug_word(word: &str) {
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
        let result = lookup(word).unwrap();
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
