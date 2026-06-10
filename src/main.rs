use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use lindera::LinderaResult;

use phf::phf_map;

#[derive(Debug, Clone, Copy)]
enum PartOfSpeech {
    Noun,
    Verb,
    AuxiliaryVerb,
    Particle,
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

static POS_SUB1: phf::Map<&'static str, '


struct Token {
    surface: String,
    pos: PartOfSpeech,
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
            parsed.push(Token {
                surface: surface,
                pos: *POS_MAP.get(details[0]).unwrap(),
            });
            println!("{:?}", details);
        }

        Ok(parsed)
    }
}


fn main() -> LinderaResult<()> {
    let parser = Parser::new().unwrap();
    let result = parser.parse("私は食べる").unwrap();
    for i in 0..result.len() {
        println!("{:?}", result[i].pos);
    }

    Ok(())
}
