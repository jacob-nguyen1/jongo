use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

const JMNEDICT_URL: &str = "https://github.com/scriptin/jmdict-simplified/releases/download/3.6.2%2B20260622163854/jmnedict-all-3.6.2%2B20260622163854.json.tgz";
const JMNEDICT_SHA256: &str = "eeee64b5c8fc2836a30ad15fe83f4e6f130e45fdb93e8a084afdfcd8d47f19db";
const JMNEDICT_JSON_NAME: &str = "jmnedict-all-3.6.2.json";

const JMDICT_URL: &str = "https://github.com/scriptin/jmdict-simplified/releases/download/3.6.2%2B20260622163854/jmdict-eng-3.6.2%2B20260622163854.json.tgz";
const JMDICT_JSON_NAME: &str = "jmdict-eng-3.6.2.json";

/// Name types we keep in the embedded subset (see jmdict-simplified / EDRDG tags).
const ALLOWED_TYPES: &[&str] = &[
    "surname",
    "place",
    "station",
    "person",
    "masc",
    "fem",
    "given",
    "unclass",
    "company",
    "organization",
    "product",
    "work",
];

#[derive(Debug, Deserialize)]
struct JmnedictFile {
    words: Vec<JmnedictWord>,
}

#[derive(Debug, Deserialize)]
struct JmnedictWord {
    kanji: Vec<JmnedictKanji>,
    kana: Vec<JmnedictKana>,
    translation: Vec<JmnedictTranslation>,
}

#[derive(Debug, Deserialize)]
struct JmnedictKanji {
    text: String,
}

#[derive(Debug, Deserialize)]
struct JmnedictKana {
    text: String,
}

#[derive(Debug, Deserialize)]
struct JmnedictTranslation {
    #[serde(rename = "type")]
    name_types: Vec<String>,
    translation: Vec<JmnedictTranslationText>,
}

#[derive(Debug, Deserialize)]
struct JmnedictTranslationText {
    lang: String,
    text: String,
}

/// Compact entry written to jmnedict.bin (must match runtime `NameEntry` in src/jmnedict.rs).
#[derive(Debug, serde::Serialize)]
struct NameData {
    entries: Vec<NameEntry>,
    by_form: Vec<(String, Vec<u32>)>,
}

#[derive(Debug, serde::Serialize)]
struct NameEntry {
    kanji: Vec<String>,
    kana: String,
    glosses: Vec<String>,
    name_type: u8,
}

#[derive(Debug, Deserialize)]
struct JmdictFile {
    words: Vec<JmdictWord>,
}

#[derive(Debug, Deserialize)]
struct JmdictWord {
    #[serde(default)]
    kanji: Vec<JmdictKanji>,
    #[serde(default)]
    kana: Vec<JmdictKana>,
    #[serde(default)]
    sense: Vec<JmdictSense>,
}

#[derive(Debug, Deserialize)]
struct JmdictKanji {
    text: String,
}

#[derive(Debug, Deserialize)]
struct JmdictKana {
    text: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JmdictSense {
    #[serde(rename = "partOfSpeech", default)]
    part_of_speech: Vec<String>,
    #[serde(default)]
    gloss: Vec<JmdictGloss>,
}

#[derive(Debug, Deserialize)]
struct JmdictGloss {
    lang: String,
    text: String,
}

#[derive(Debug, serde::Serialize)]
struct JmdictData {
    entries: Vec<JmdictEntry>,
    by_form: Vec<(String, Vec<u32>)>,
}

#[derive(Debug, serde::Serialize)]
struct JmdictEntry {
    kana: String,
    glosses: Vec<String>,
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("jmnedict.bin");

    println!("cargo:rerun-if-env-changed=JONGO_JMNEDICT_PATH");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=custom_dict.csv");

    let json_path = acquire_json(JMNEDICT_URL, JMNEDICT_JSON_NAME, Some(JMNEDICT_SHA256), "JONGO_JMNEDICT_PATH");
    let entries = process_json(&json_path);

    let mut by_form_map: std::collections::HashMap<String, Vec<u32>> = std::collections::HashMap::new();
    for (idx, entry) in entries.iter().enumerate() {
        let idx = idx as u32;
        for kanji in &entry.kanji {
            by_form_map.entry(kanji.clone()).or_default().push(idx);
        }
        by_form_map.entry(entry.kana.clone()).or_default().push(idx);
    }
    let mut by_form: Vec<(String, Vec<u32>)> = by_form_map.into_iter().collect();
    by_form.sort_by(|a, b| a.0.cmp(&b.0));

    let name_data = NameData { entries, by_form };

    let mut f = File::create(&dest).unwrap();
    let bytes = postcard::to_allocvec(&name_data).unwrap();
    std::io::Write::write_all(&mut f, &bytes).unwrap();

    let dest_jmdict = out_dir.join("jmdict.bin");
    let json_path_jmdict = acquire_json(JMDICT_URL, JMDICT_JSON_NAME, None, "JONGO_JMDICT_PATH");
    let (jmdict_entries, jmdict_by_form) = process_jmdict_json(&json_path_jmdict);
    
    let jmdict_data = JmdictData { entries: jmdict_entries, by_form: jmdict_by_form };
    let jmdict_bytes = postcard::to_allocvec(&jmdict_data).expect("failed to serialize jmdict.bin");
    fs::write(&dest_jmdict, &jmdict_bytes).expect("failed to write jmdict.bin");

    eprintln!(
        "jmnedict: packed {} entries ({} bytes)",
        name_data.entries.len(),
        bytes.len()
    );

    // === Custom user dictionary for Lindera ===
    let custom_csv = Path::new("custom_dict.csv");
    let custom_bin = out_dir.join("custom_dict.bin");
    if custom_csv.exists() {
        use lindera_dictionary::dictionary::metadata::Metadata;
        use lindera_dictionary::builder::DictionaryBuilder;

        // Use the same metadata.json that lindera-ipadic uses
        let metadata_json = r#"{
            "name": "ipadic",
            "encoding": "UTF-8",
            "default_word_cost": -10000,
            "default_left_context_id": 0,
            "default_right_context_id": 0,
            "default_field_value": "*",
            "flexible_csv": true,
            "skip_invalid_cost_or_id": false,
            "normalize_details": true,
            "dictionary_schema": {
                "fields": [
                    "surface", "left_context_id", "right_context_id", "cost",
                    "part_of_speech", "part_of_speech_subcategory_1",
                    "part_of_speech_subcategory_2", "part_of_speech_subcategory_3",
                    "conjugation_form", "conjugation_type",
                    "base_form", "reading", "pronunciation"
                ]
            },
            "user_dictionary_schema": {
                "fields": ["surface", "part_of_speech", "reading"]
            }
        }"#;
        let metadata: Metadata = serde_json::from_str(metadata_json)
            .expect("failed to parse IPADIC metadata for custom dict");
        let builder = DictionaryBuilder::new(metadata);
        builder.build_user_dictionary(custom_csv, &custom_bin)
            .expect("failed to build custom user dictionary from custom_dict.csv");
        eprintln!("custom_dict: compiled custom_dict.csv -> custom_dict.bin");
    } else {
        // Write an empty file so include_bytes! doesn't fail
        fs::write(&custom_bin, &[]).expect("failed to write empty custom_dict.bin");
        eprintln!("custom_dict: no custom_dict.csv found, writing empty bin");
    }
}

fn acquire_json(url: &str, json_name: &str, expected_sha: Option<&str>, env_override: &str) -> PathBuf {
    if let Ok(path) = env::var(env_override) {
        let path = PathBuf::from(path);
        if !path.exists() {
            panic!("{} does not exist: {}", env_override, path.display());
        }
        return path;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let cache_dir = out_dir.join("dict-cache");
    fs::create_dir_all(&cache_dir).unwrap();
    let json_path = cache_dir.join(json_name);

    if json_path.exists() {
        return json_path;
    }

    eprintln!("downloading {url}");
    let response = ureq::get(url)
        .call()
        .unwrap_or_else(|e| panic!("failed to download dict: {e}"));
    let mut archive_bytes = Vec::new();
    response
        .into_body()
        .into_with_config()
        .limit(50_000_000)
        .reader()
        .read_to_end(&mut archive_bytes)
        .expect("failed to read archive");

    if let Some(sha) = expected_sha {
        let digest = Sha256::digest(&archive_bytes).iter().map(|b| format!("{:02x}", b)).collect::<String>();
        if digest != sha {
            panic!("archive checksum mismatch: expected {sha}, got {digest}");
        }
    }

    let mut archive = flate2::read::GzDecoder::new(&archive_bytes[..]);
    let mut tar = tar::Archive::new(&mut archive);
    tar.entries()
        .expect("failed to read tar entries")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .ok()
                .and_then(|p| p.file_name().map(|n| n == json_name))
                .unwrap_or(false)
        })
        .for_each(|mut entry| {
            entry
                .unpack(&json_path)
                .expect("failed to extract JSON");
        });

    if !json_path.exists() {
        panic!("JSON not found in archive");
    }
    json_path
}

fn process_jmdict_json(path: &Path) -> (Vec<JmdictEntry>, Vec<(String, Vec<u32>)>) {
    let file = File::open(path).expect("failed to open JMdict JSON");
    let reader = BufReader::new(file);
    let data: JmdictFile = serde_json::from_reader(reader).expect("failed to parse JMdict JSON");

    let mut entries = Vec::new();
    let mut by_form_map: std::collections::HashMap<String, Vec<u32>> = std::collections::HashMap::new();

    for word in data.words {
        let mut glosses = Vec::new();
        for sense in &word.sense {
            for g in &sense.gloss {
                if g.lang == "eng" {
                    glosses.push(g.text.clone());
                }
            }
        }
        if glosses.is_empty() { continue; }
        if word.kana.is_empty() { continue; }
        
        let entry = JmdictEntry {
            kana: word.kana[0].text.clone(),
            glosses,
        };
        
        let idx = entries.len() as u32;
        entries.push(entry);
        
        for k in &word.kanji {
            by_form_map.entry(k.text.clone()).or_default().push(idx);
        }
        for r in &word.kana {
            by_form_map.entry(r.text.clone()).or_default().push(idx);
        }
    }

    let mut by_form: Vec<(String, Vec<u32>)> = by_form_map.into_iter().collect();
    by_form.sort_by(|a, b| a.0.cmp(&b.0));
    
    (entries, by_form)
}

fn process_json(path: &Path) -> Vec<NameEntry> {
    let file = File::open(path).expect("failed to open JMnedict JSON");
    let reader = BufReader::new(file);
    let data: JmnedictFile = serde_json::from_reader(reader).expect("failed to parse JMnedict JSON");

    let allowed: HashSet<&str> = ALLOWED_TYPES.iter().copied().collect();
    let mut entries = Vec::new();

    for word in data.words {
        let mut glosses = Vec::new();
        let mut name_type: Option<u8> = None;

        for trans in &word.translation {
            if !trans.name_types.iter().any(|t| allowed.contains(t.as_str())) {
                continue;
            }
            if name_type.is_none() {
                name_type = trans
                    .name_types
                    .iter()
                    .find_map(|t| classify_name_type_code(t));
            }
            for g in &trans.translation {
                if is_english(&g.lang) && !g.text.is_empty() {
                    glosses.push(g.text.clone());
                }
            }
        }

        glosses.sort();
        glosses.dedup();
        if glosses.is_empty() {
            continue;
        }

        let kana = word
            .kana
            .first()
            .map(|k| k.text.clone())
            .unwrap_or_else(|| {
                word.kanji
                    .first()
                    .map(|k| k.text.clone())
                    .unwrap_or_default()
            });
        if kana.is_empty() {
            continue;
        }

        let kanji: Vec<String> = word.kanji.into_iter().map(|k| k.text).collect();
        entries.push(NameEntry {
            kanji,
            kana,
            glosses,
            name_type: name_type.unwrap_or(5), // Other
        });
    }

    entries
}

fn is_english(lang: &str) -> bool {
    lang.is_empty() || lang == "eng"
}

/// Maps jmdict-simplified name-type tags to `NameTypeCode` (must match src/jmnedict.rs).
fn classify_name_type_code(tag: &str) -> Option<u8> {
    Some(match tag {
        "surname" | "person" | "masc" | "fem" | "given" | "unclass" => 0, // Person
        "place" | "station" => 1,                                         // Place
        "company" | "organization" => 2,                                  // Organization
        "product" => 3,                                                   // Product
        "work" => 4,                                                      // Work
        _ => return None,
    })
}
