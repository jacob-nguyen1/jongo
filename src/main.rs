use std::sync::LazyLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use lindera::LinderaResult;

use phf::phf_map;

#[derive(Debug, Clone, Copy)]
enum PartOfSpeech {
    Noun,
    Prefix,
    Verb,
    Adjective,
    Adverb,
    AdnominalAdjective,
    Conjunction,
    Particle,
    AuxiliaryVerb,
    Interjection,
    Symbol,
    Filler,
    Others,
}

static POS_MAP: phf::Map<&'static str, PartOfSpeech> = phf_map! {
    "名詞" => PartOfSpeech::Noun,
    "接頭詞" => PartOfSpeech::Prefix,
    "動詞" => PartOfSpeech::Verb,
    "形容詞" => PartOfSpeech::Adjective,
    "副詞" => PartOfSpeech::Adverb,
    "連体詞" => PartOfSpeech::AdnominalAdjective,
    "接続詞" => PartOfSpeech::Conjunction,
    "助詞" => PartOfSpeech::Particle,
    "助動詞" => PartOfSpeech::AuxiliaryVerb,
    "感動詞" => PartOfSpeech::Interjection,
    "記号" => PartOfSpeech::Symbol,
    "フィラー" => PartOfSpeech::Filler,
    "その他" => PartOfSpeech::Others,
};

#[derive(Debug, Clone, Copy)]
enum PartOfSpeechSubcategory1 {
    SuruVerb,
    NaiAdjStem,
    General,
    QuoteIndicator,
    ProperNoun,
    Number,
    Conjunction,
    Suffix,
    Pronoun,
    DependentVerb,
    Irregular,
    Bound,
    Adverbial,
    AdjectiveConjunction,
    NumeralPrefix,
    VerbConnection,
    NounConnection,
    Unbound,
    ParticleConnectingAdverb,
    MarkingParticle,
    LinkingParticle,
    EndingParticle,
    ConjuctiveParticle,
    AdverbializingParticle,
    AdverbialParticle,
    AdverbialORCoordinatingOREndingParticle,
    CoordinatingParticle,
    NormalizingParticle,
    Alphabet,
    OpenParenthesis,
    ClosedParenthesis,
    Period,
    Void,
    Comma,
    Interjection,
    X,
}

static POS_SUB1_MAP: phf::Map<&'static str, PartOfSpeechSubcategory1> = phf_map! {
    "サ変接続" => PartOfSpeechSubcategory1::SuruVerb,
    "ナイ形容詞語幹" => PartOfSpeechSubcategory1::NaiAdjStem,
    "一般" => PartOfSpeechSubcategory1::General,
    "引用文字列" => PartOfSpeechSubcategory1::QuoteIndicator,
    "固有名詞" => PartOfSpeechSubcategory1::ProperNoun,
    "数" => PartOfSpeechSubcategory1::Number,
    "接続詞的" => PartOfSpeechSubcategory1::Conjunction,
    "接尾" => PartOfSpeechSubcategory1::Suffix,
    "代名詞" => PartOfSpeechSubcategory1::Pronoun,
    "動詞非自立的" => PartOfSpeechSubcategory1::DependentVerb,
    "特殊" => PartOfSpeechSubcategory1::Irregular,
    "非自立" => PartOfSpeechSubcategory1::Bound,
    "副詞可能" => PartOfSpeechSubcategory1::Adverbial,
    "形容詞接続" => PartOfSpeechSubcategory1::AdjectiveConjunction,
    "数接続" => PartOfSpeechSubcategory1::NumeralPrefix,
    "動詞接続" => PartOfSpeechSubcategory1::VerbConnection,
    "名詞接続" => PartOfSpeechSubcategory1::NounConnection,
    "自立" => PartOfSpeechSubcategory1::Unbound,
    "助詞類接続" => PartOfSpeechSubcategory1::ParticleConnectingAdverb,
    "格助詞" => PartOfSpeechSubcategory1::MarkingParticle,
    "係助詞" => PartOfSpeechSubcategory1::LinkingParticle,
    "終助詞" => PartOfSpeechSubcategory1::EndingParticle,
    "接続助詞" => PartOfSpeechSubcategory1::ConjuctiveParticle,
    "副詞化" => PartOfSpeechSubcategory1::AdverbializingParticle,
    "副助詞" => PartOfSpeechSubcategory1::AdverbialParticle,
    "副助詞／並立助詞／終助詞" => PartOfSpeechSubcategory1::AdverbialORCoordinatingOREndingParticle,
    "並立助詞" => PartOfSpeechSubcategory1::CoordinatingParticle,
    "連体化" => PartOfSpeechSubcategory1::NormalizingParticle,
    "アルファベット" => PartOfSpeechSubcategory1::Alphabet,
    "括弧開" => PartOfSpeechSubcategory1::OpenParenthesis,
    "括弧閉" => PartOfSpeechSubcategory1::ClosedParenthesis,
    "句点" => PartOfSpeechSubcategory1::Period,
    "空白" => PartOfSpeechSubcategory1::Void,
    "読点" => PartOfSpeechSubcategory1::Comma,
    "間投" => PartOfSpeechSubcategory1::Interjection,
    "*" => PartOfSpeechSubcategory1::X,
};

#[derive(Debug, Clone, Copy)]
enum PartOfSpeechSubcategory2 {
    General,
    Name,
    Organization,
    Region,
    SuruVerb,
    AdjectiveVerbStem,
    Counter,
    AuxilaryVerbStem,
    Irregular,
    PossibleAdverb,
    Contraction,
    Quotation,
    Collocation,
    X,
}

static POS_SUB2_MAP: phf::Map<&'static str, PartOfSpeechSubcategory2> = phf_map! {
    "一般" => PartOfSpeechSubcategory2::General,
    "人名" => PartOfSpeechSubcategory2::Name,
    "組織" => PartOfSpeechSubcategory2::Organization,
    "地域" => PartOfSpeechSubcategory2::Region,
    "サ変接続" => PartOfSpeechSubcategory2::SuruVerb,
    "形容動詞語幹" => PartOfSpeechSubcategory2::AdjectiveVerbStem,
    "助数詞" => PartOfSpeechSubcategory2::Counter,
    "助動詞語幹" => PartOfSpeechSubcategory2::AuxilaryVerbStem,
    "特殊" => PartOfSpeechSubcategory2::Irregular,
    "副詞可能" => PartOfSpeechSubcategory2::PossibleAdverb,
    "縮約" => PartOfSpeechSubcategory2::Contraction,
    "引用" => PartOfSpeechSubcategory2::Quotation,
    "連語" => PartOfSpeechSubcategory2::Collocation,
    "*" => PartOfSpeechSubcategory2::X,
};

#[derive(Debug, Clone, Copy)]
enum PartOfSpeechSubcategory3 {
    General,
    Surname,
    Name,
    Country,
    X,
}

static POS_SUB3_MAP: phf::Map<&'static str, PartOfSpeechSubcategory3> = phf_map! {
    "一般" => PartOfSpeechSubcategory3::General,
    "姓" => PartOfSpeechSubcategory3::Surname,
    "名" => PartOfSpeechSubcategory3::Name,
    "国" => PartOfSpeechSubcategory3::Country,
    "*" => PartOfSpeechSubcategory3::X,
};

struct Token {
    surface: String,
    pos: PartOfSpeech,
    sub1: PartOfSpeechSubcategory1,
    sub2: PartOfSpeechSubcategory2,
    sub3: PartOfSpeechSubcategory3,
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
                pos: *POS_MAP.get(details[0]).unwrap(),
                sub1: *POS_SUB1_MAP.get(details[1]).unwrap(),
                sub2: *POS_SUB2_MAP.get(details[2]).unwrap(),
                sub3: *POS_SUB3_MAP.get(details[3]).unwrap(),
            }
        }).collect();

        Ok(parsed_tokens)
    }
}


fn main() {
    let result = PARSER.parse("私は明日使ったご飯を食べています").unwrap();

    result.iter().for_each(|t| {
        println!("{}, {:?}, {:?}, {:?}, {:?}", t.surface, t.pos, t.sub1, t.sub2, t.sub3);
    });

}