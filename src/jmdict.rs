use jmdict::{entries, GlossLanguage};

pub fn lookup(word: &str) -> Option<String> {
    let entry = entries().find(|e| {e.kanji_elements().any(|k| k.text == word) || e.reading_elements().any(|r| r.text == word)})?;

    // get the kana reading
    let kana = entry.reading_elements().next()?.text;

    // collect all English glosses
    let glosses: Vec<&str> = entry.senses().flat_map(|s| s.glosses()).filter(|g| g.language == GlossLanguage::English).map(|g| g.text).collect();

    Some(format!("kana: {}\nenglish: {}", kana, glosses.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_okaasan() {
        let word = "お母さん";
        let result = lookup(word).unwrap();
        println!("original word: {}\n{}", word, result);
        assert!(result.contains("おかあさん"));
        assert!(result.contains("mother"));
    }
}