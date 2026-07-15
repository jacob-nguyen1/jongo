use std::collections::HashMap;
use std::sync::LazyLock;

/// Must match `NameEntry` in build.rs (postcard schema).
#[derive(Debug, serde::Deserialize)]
struct NameEntry {
    kanji: Vec<String>,
    kana: String,
    glosses: Vec<String>,
    name_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProperNounType {
    Person,
    Place,
    Organization,
    Product,
    Work,
    Other,
    NotApplicable,
}

impl ProperNounType {
    pub fn label(&self) -> &'static str {
        match self {
            ProperNounType::Person => "person name",
            ProperNounType::Place => "place name",
            ProperNounType::Organization => "organization",
            ProperNounType::Product => "product",
            ProperNounType::Work => "work",
            ProperNounType::Other => "name",
            ProperNounType::NotApplicable => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NameHit {
    pub kana: String,
    pub glosses: Vec<String>,
    pub noun_type: ProperNounType,
}

struct NameIndex {
    entries: Vec<NameEntry>,
    by_form: HashMap<String, Vec<u32>>,
}

static INDEX: LazyLock<NameIndex> = LazyLock::new(NameIndex::load);

impl NameIndex {
    fn load() -> Self {
        let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/jmnedict.bin"));
        let entries: Vec<NameEntry> =
            postcard::from_bytes(bytes).expect("failed to deserialize jmnedict.bin");

        let mut by_form: HashMap<String, Vec<u32>> = HashMap::new();
        for (idx, entry) in entries.iter().enumerate() {
            let idx = idx as u32;
            for kanji in &entry.kanji {
                by_form.entry(kanji.clone()).or_default().push(idx);
            }
            by_form.entry(entry.kana.clone()).or_default().push(idx);
        }

        Self { entries, by_form }
    }
}

fn decode_name_type(code: u8) -> ProperNounType {
    match code {
        0 => ProperNounType::Person,
        1 => ProperNounType::Place,
        2 => ProperNounType::Organization,
        3 => ProperNounType::Product,
        4 => ProperNounType::Work,
        _ => ProperNounType::Other,
    }
}

/// Look up a proper noun by kanji or kana reading.
pub fn lookup_name(word: &str) -> Vec<NameHit> {
    let Some(indices) = INDEX.by_form.get(word) else {
        return Vec::new();
    };

    let mut hits = Vec::new();
    let mut seen = Vec::new();

    for &idx in indices {
        if seen.contains(&idx) {
            continue;
        }
        seen.push(idx);

        let entry = &INDEX.entries[idx as usize];
        hits.push(NameHit {
            kana: entry.kana.clone(),
            glosses: entry.glosses.clone(),
            noun_type: decode_name_type(entry.name_type),
        });
    }

    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_tokyo() {
        let hits = lookup_name("東京");
        assert!(!hits.is_empty(), "expected 東京 in JMnedict");
        assert!(
            matches!(hits[0].noun_type, ProperNounType::Place),
            "expected place type, got {:?}",
            hits[0].noun_type
        );
    }

    #[test]
    fn lookup_station() {
        let hits = lookup_name("新宿");
        assert!(!hits.is_empty(), "expected Shinjuku in JMnedict");
    }
}
