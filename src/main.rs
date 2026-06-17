<<<<<<< Updated upstream
use std::sync::LazyLock;

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
    Unknown,
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

static PARSER: LazyLock<Parser> = LazyLock::new(|| Parser::new().unwrap());

impl Parser {
    fn new() -> LinderaResult<Self> {
        let dictionary = load_dictionary("embedded://ipadic")?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let tokenizer = Tokenizer::new(segmenter);
        Ok(Self { tokenizer })
    }

    fn parse(&self, text: &str) -> LinderaResult<Vec<Token>> {
        let tokens = self.tokenizer.tokenize(text)?;
        let parsed_tokens: Vec<Token> = tokens.into_iter().map(|mut token| {
            let surface = token.surface.to_string();
            let details = token.details();
            println!("{:?}", details);
            
            Token {
                surface: surface,
                pos: *POS_MAP.get(details[0]).unwrap_or(&PartOfSpeech::Unknown),
            }
        }).collect();

        Ok(parsed_tokens)
    }
}


fn main() {
    let result = PARSER.parse("公園で遊んでいた子どもたちが、急に降り始めた雨を見て、近くの店まで走って行った。").unwrap();

    result.iter().for_each(|t| {
        println!("{} {:?}",t.surface, t.pos);
    });

=======

mod grammar;
fn main() {
    grammar::grammar();
>>>>>>> Stashed changes
}
