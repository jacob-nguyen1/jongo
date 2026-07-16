use std::sync::LazyLock;

#[derive(Debug, serde::Deserialize)]
struct NameData<'a> {
    #[serde(borrow)]
    entries: Vec<NameEntry<'a>>,
    #[serde(borrow)]
    by_form: Vec<(&'a str, Vec<u32>)>,
}

/// Must match `NameData` and `NameEntry` in build.rs (postcard schema).
#[derive(Debug, serde::Deserialize)]
struct NameEntry<'a> {
    #[serde(borrow)]
    kanji: Vec<&'a str>,
    kana: &'a str,
    #[serde(borrow)]
    glosses: Vec<&'a str>,
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

static JMNEDICT_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/jmnedict.bin"));
static INDEX: LazyLock<NameData<'static>> = LazyLock::new(|| postcard::from_bytes(JMNEDICT_BYTES).expect("failed to deserialize jmnedict.bin"));

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
    let Ok(idx_in_by_form) = INDEX.by_form.binary_search_by_key(&word, |&(k, _)| k) else {
        return Vec::new();
    };
    let indices = &INDEX.by_form[idx_in_by_form].1;

    let mut hits = Vec::new();
    let mut seen = Vec::new();

    for &idx in indices {
        if seen.contains(&idx) {
            continue;
        }
        seen.push(idx);

        let entry = &INDEX.entries[idx as usize];
        hits.push(NameHit {
            kana: entry.kana.to_string(),
            glosses: entry.glosses.iter().map(|s| s.to_string()).collect(),
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
