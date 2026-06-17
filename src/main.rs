use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use lindera::LinderaResult;

mod jmdict;
use jmdict::lookup;

use phf::phf_map;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartOfSpeech {
    Noun,
    Verb,
    AuxiliaryVerb,
    Particle,
}

impl PartOfSpeech {
    pub fn matches_jmdict(&self, pos: &::jmdict::PartOfSpeech) -> bool {
        let pos_str = format!("{:?}", pos);
        match self {
            Self::Noun => pos_str.contains("Noun") || pos_str.contains("Pronoun") || pos_str.contains("Numeric"),
            Self::Verb => pos_str.contains("Verb"),
            Self::AuxiliaryVerb => pos_str.contains("Verb") || pos_str.contains("Auxiliary") || pos_str.contains("Copula"),
            Self::Particle => pos_str.contains("Particle"),
        }
    }
}

static POS_MAP: phf::Map<&'static str, PartOfSpeech> = phf_map! {
    "名詞" => PartOfSpeech::Noun,
    "動詞" => PartOfSpeech::Verb,
    "助動詞" => PartOfSpeech::AuxiliaryVerb,
    "助詞" => PartOfSpeech::Particle,
};

#[derive(Debug, Clone, Copy)]
enum PartOfSpeechSubcategory1 {
    Noun,
    Verb,
    AuxiliaryVerb,
    Particle,
}

struct Token {
    surface: String,
    pos: PartOfSpeech,
    kana: String,
    glosses: Vec<String>,
}

struct Parser {
    tokenizer: Tokenizer,
}

impl Parser {
    fn new() -> LinderaResult<Self> {
        let dictionary = load_dictionary("embedded://ipadic")?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let tokenizer = Tokenizer::new(segmenter);
        Ok(Self { tokenizer })
    }

    fn parse<'a>(&'a self, text: &'a str) -> LinderaResult<Vec<Token>> {
        let mut tokens = self.tokenizer.tokenize(text)?;
        let mut parsed: Vec<Token> = Vec::new();
        for mut token in tokens {
            let surface = token.surface.to_string();
            let details = token.details();
            let pos = *POS_MAP.get(details[0]).unwrap();
            let Some((kana, glosses)) = lookup(&surface, pos) else {
                println!("Lookup failed for {}", surface);
                continue;
            }; 
            parsed.push(Token {
                surface: surface,
                pos: *POS_MAP.get(details[0]).unwrap(),
                kana: kana,
                glosses: glosses,
            });
            println!("{:?}", details); // prints lindera token in vector format
        }
        println!();

        Ok(parsed)
    }
}


fn main() -> LinderaResult<()> {
    let parser = Parser::new().unwrap();
    let result = parser.parse("私は食べる").unwrap();
    for i in 0..result.len() {
        println!("Word {}", i);
        println!("Kana: {}", result[i].kana);
        println!("English: {:?}", result[i].glosses);
        println!("POS: {:?}", result[i].pos); // prints part of speech
        println!("---------------");
    }

    Ok(())
}
