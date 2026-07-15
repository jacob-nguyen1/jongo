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
struct NameEntry {
    kanji: Vec<String>,
    kana: String,
    glosses: Vec<String>,
    name_type: u8,
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("jmnedict.bin");

    println!("cargo:rerun-if-env-changed=JONGO_JMNEDICT_PATH");
    println!("cargo:rerun-if-changed=build.rs");

    let json_path = acquire_json();
    let entries = process_json(&json_path);
    let encoded = postcard::to_allocvec(&entries).expect("failed to serialize jmnedict.bin");
    fs::write(&dest, &encoded).expect("failed to write jmnedict.bin");
    eprintln!(
        "jmnedict: packed {} entries ({} bytes)",
        entries.len(),
        encoded.len()
    );
}

fn acquire_json() -> PathBuf {
    if let Ok(path) = env::var("JONGO_JMNEDICT_PATH") {
        let path = PathBuf::from(path);
        if !path.exists() {
            panic!("JONGO_JMNEDICT_PATH does not exist: {}", path.display());
        }
        return path;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let cache_dir = out_dir.join("jmnedict-cache");
    fs::create_dir_all(&cache_dir).unwrap();
    let json_path = cache_dir.join(JMNEDICT_JSON_NAME);

    if json_path.exists() {
        return json_path;
    }

    eprintln!("jmnedict: downloading {JMNEDICT_URL}");
    let response = ureq::get(JMNEDICT_URL)
        .call()
        .unwrap_or_else(|e| panic!("failed to download JMnedict: {e}"));
    let mut archive_bytes = Vec::new();
    response
        .into_body()
        .into_with_config()
        .limit(50_000_000)
        .reader()
        .read_to_end(&mut archive_bytes)
        .expect("failed to read JMnedict archive");

    let digest = Sha256::digest(&archive_bytes).iter().map(|b| format!("{:02x}", b)).collect::<String>();
    if digest != JMNEDICT_SHA256 {
        panic!(
            "JMnedict archive checksum mismatch: expected {JMNEDICT_SHA256}, got {digest}"
        );
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
                .and_then(|p| p.file_name().map(|n| n == JMNEDICT_JSON_NAME))
                .unwrap_or(false)
        })
        .for_each(|mut entry| {
            entry
                .unpack(&json_path)
                .expect("failed to extract JMnedict JSON");
        });

    if !json_path.exists() {
        panic!("JMnedict JSON not found in archive");
    }
    json_path
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
