use lindera::LinderaResult;
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use phf::phf_map;
use std::sync::LazyLock;

use crate::grammar::Conjugations::PresentContinuous;

// use crate::grammar::CForms::Continuative;
// use crate::grammar::PartOfSpeech::Verb;
// use crate::grammar::PartOfSpeechSubcategory1::ConjuctiveParticle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartOfSpeech {
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
    ERR,
}

impl PartOfSpeech {
    pub fn matches_jmdict(&self, p: &jmdict::PartOfSpeech) -> bool {
        let p_str = format!("{:?}", p).to_lowercase();
        match self {
            PartOfSpeech::Noun => p_str.contains("noun"),
            PartOfSpeech::Verb => p_str.contains("verb"),
            PartOfSpeech::Adjective => p_str.contains("adjective"),
            PartOfSpeech::Adverb => p_str.contains("adverb"),
            PartOfSpeech::Particle => p_str.contains("particle"),
            _ => true,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartOfSpeechSubcategory1 {
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
    AdjectiveVerbStem,
    X,
    ERR,
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
    "形容動詞語幹" => PartOfSpeechSubcategory1::AdjectiveVerbStem,
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
    ERR,
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
    ERR,
}

static POS_SUB3_MAP: phf::Map<&'static str, PartOfSpeechSubcategory3> = phf_map! {
    "一般" => PartOfSpeechSubcategory3::General,
    "姓" => PartOfSpeechSubcategory3::Surname,
    "名" => PartOfSpeechSubcategory3::Name,
    "国" => PartOfSpeechSubcategory3::Country,
    "*" => PartOfSpeechSubcategory3::X,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CTypes {
    AdjectiveAUODan,
    AdjectiveII,
    AdjectiveIDan,
    Invariable,
    IrregularKuRu,
    DependentSuRuConnection,
    DependentZuRuConnection,
    IndependentSuruConnection,
    IrregularRaVerb,
    RuVerb,
    KuReRu,
    HaRowUEConjugation,
    IrregularERu,
    IrregularIKuTeForm,
    BuUVerb,
    MuUVerb,
    RuUVerb,
    WaUVerb,
    TsuVerb,
    AruVerb,
    KeigoAruVerb,
    IrregularTa,
    IrregularTai,
    IrregularNu,
    IrregularMaSu,
    IrregularNai,
    ClassicalKi,
    ClassicalBeShi,
    ClassicalRu,
    X,
    ERR,
}

static C_TYPE_MAP: phf::Map<&'static str, CTypes> = phf_map! {
    "形容詞・アウオ段" => CTypes::AdjectiveAUODan,
    "形容詞・イイ" => CTypes::AdjectiveII,
    "形容詞・イ段" => CTypes::AdjectiveIDan,
    "不変化型" => CTypes::Invariable,
    "カ変・来ル" => CTypes::IrregularKuRu,
    "サ変・−スル" => CTypes::DependentSuRuConnection,
    "サ変・−ズル" => CTypes::DependentZuRuConnection,
    "サ変・スル" => CTypes::IndependentSuruConnection,
    "ラ変" => CTypes::IrregularRaVerb,
    "一段" => CTypes::RuVerb,
    "一段・クレル" => CTypes::KuReRu,
    "下二・ハ行" => CTypes::HaRowUEConjugation,
    "下二・得" => CTypes::IrregularERu,
    "五段・カ行促音便ユク" => CTypes::IrregularIKuTeForm,
    "五段・バ行" => CTypes::BuUVerb,
    "五段・マ行" => CTypes::MuUVerb,
    "五段・ラ行" => CTypes::RuUVerb,
    "五段・ワ行促音便" => CTypes::WaUVerb,
    "下二・タ行" => CTypes::TsuVerb,
    "五段・ラ行アル" => CTypes::AruVerb,
    "五段・ラ行特殊" => CTypes::KeigoAruVerb,
    "特殊・タ" => CTypes::IrregularTa,
    "特殊・タイ" => CTypes::IrregularTai,
    "特殊・ヌ" => CTypes::IrregularNu,
    "特殊・マス" => CTypes::IrregularMaSu,
    "特殊・ナイ" => CTypes::IrregularNai,
    "文語・キ" => CTypes::ClassicalKi,
    "文語・ベシ" => CTypes::ClassicalBeShi,
    "文語・ル" => CTypes::ClassicalRu,
    "*" => CTypes::X,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CForms {
    ImperativeYo,
    AReruConnection,
    Plain,
    ClassicalPlain,
    NounConnection,
    ContractedRuNNo,
    AUConnection,
    ContractedRaNNai,
    ContractedRuNo,
    Garu,
    GozaiConjunction,
    DoubleConsonant,
    Continuative,
    Imperfective,
    Continuative2,
    X,
    ERR,
}

static C_FORM_MAP: phf::Map<&'static str, CForms> = phf_map! {
    "命令ｙｏ" => CForms::ImperativeYo,
    "未然レル接続" => CForms::AReruConnection,
    "基本形" => CForms::Plain,
    "文語基本形" => CForms::ClassicalPlain,
    "体言接続" => CForms::NounConnection,
    "体言接続特殊" => CForms::ContractedRuNNo,
    "未然ウ接続" => CForms::AUConnection,
    "未然特殊" => CForms::ContractedRaNNai,
    "体言接続特殊２" => CForms::ContractedRuNo,
    "ガル接続" => CForms::Garu,
    "連用ゴザイ接続" => CForms::GozaiConjunction,
    "基本形-促音便" => CForms::DoubleConsonant,
    "連用形" => CForms::Continuative,
    "未然形" => CForms::Imperfective,
    "連用タ接続" => CForms::Continuative,
    "*" => CForms::X,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Conjugations {
    NonPast,
    PresentContinuous,
    Past,
    PastContinuous,
}

struct Token {
    surface: String,
    pos: PartOfSpeech,
    sub1: PartOfSpeechSubcategory1,
    sub2: PartOfSpeechSubcategory2,
    sub3: PartOfSpeechSubcategory3,
    detail: Vec<String>,
    base: String,
    ctype: CTypes,
    cform: CForms,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConjugationFeatures {
    pub negative: bool,
    pub past: bool,
    pub continuous: bool,
}

#[derive(Debug, Clone)]
pub struct ProcToken {
    pub full: String,
    pub base: String,
    pub pos: PartOfSpeech,
    pub sub1: PartOfSpeechSubcategory1,
    pub conjugation: Option<ConjugationFeatures>,
}

impl ProcToken{
    fn verbPrint(&self) -> String {
        match &self.conjugation {
            Some(conj) => {
                let mut parts = Vec::new();
                if conj.negative {
                    parts.push("negative");
                }
                if conj.past {
                    parts.push("past");
                }
                if conj.continuous {
                    parts.push("continuous");
                }
                if parts.is_empty() {
                    "none".to_string()
                } else {
                    parts.join(", ")
                }
            }
            None => "none".to_string(),
        }
    }
}

fn filter(line: &[Token]) -> Vec<ProcToken> {
    let mut filtered_tokens: Vec<ProcToken> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let token = line.get(i).unwrap();
        let pos = token.pos.clone();
        let base = token.base.clone();
        let sub1=token.sub1.clone();
        let mut conj = token.surface.clone();
        let mut negative=false;
        let mut past=false;
        let mut continuous=false;
        if token.cform == CForms::Continuative {
            let mut j=i;
            j+=1;
            while j<line.len() && (line[j].sub1==PartOfSpeechSubcategory1::ConjuctiveParticle || line[j].cform==CForms::Continuative){
                continuous=true;
                j+=1;
            }
            if j<line.len(){
                past=(line[j].ctype==CTypes::IrregularTa);
            }
        }
        if token.surface != token.base && (token.surface != "な" && token.pos != PartOfSpeech::AuxiliaryVerb){
            i += 1;
            while i < line.len()
                && line[i].pos != PartOfSpeech::Symbol
                && line[i].sub1 != PartOfSpeechSubcategory1::Unbound
            {
                conj = conj + &line[i].surface;
                i+=1;
            }
        }
        else{
            i+=1;
        }
        // let conjugation = match (past, continuous) {
        //     (true, true) => Conjugations::PastContinuous,
        //     (true, false) => Conjugations::Past,
        //     (false, true) => Conjugations::PresentContinuous,
        //     (false, false) => Conjugations::NonPast,
        // };
        filtered_tokens.push(ProcToken {
            full: conj,
            base,
            pos: pos,
            sub1,
            conjugation: Some(ConjugationFeatures {
                negative,
                past,
                continuous,
            })
        });
    }
    filtered_tokens
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
        let parsed_tokens: Vec<Token> = tokens
            .into_iter()
            .map(|mut token| {
                let surface = token.surface.to_string();
                let details = token.details();
                let detail: Vec<String> = details.iter().map(|s| (*s).to_string()).collect();

                let pos = details
                    .get(0)
                    .and_then(|k| POS_MAP.get(*k))
                    .copied()
                    .unwrap_or(PartOfSpeech::ERR);

                let sub1 = details
                    .get(1)
                    .and_then(|k| POS_SUB1_MAP.get(*k))
                    .copied()
                    .unwrap_or(PartOfSpeechSubcategory1::ERR);

                let sub2 = details
                    .get(2)
                    .and_then(|k| POS_SUB2_MAP.get(*k))
                    .copied()
                    .unwrap_or(PartOfSpeechSubcategory2::ERR);

                let sub3 = details
                    .get(3)
                    .and_then(|k| POS_SUB3_MAP.get(*k))
                    .copied()
                    .unwrap_or(PartOfSpeechSubcategory3::ERR);

                let ctype = details
                    .get(4)
                    .and_then(|k| C_TYPE_MAP.get(*k))
                    .copied()
                    .unwrap_or(CTypes::ERR);

                let cform = details
                    .get(5)
                    .and_then(|k| C_FORM_MAP.get(*k))
                    .copied()
                    .unwrap_or(CForms::ERR);

                let base = details
                    .get(6)
                    .map(|s| (*s).to_string())
                    .unwrap_or(surface.clone());

                Token {
                    surface: surface,
                    pos: pos,
                    sub1: sub1,
                    sub2: sub2,
                    sub3: sub3,
                    detail: detail,
                    base: base,
                    ctype: ctype,
                    cform: cform,
                }
            })
            .collect();

        Ok(parsed_tokens)
    }
}

pub fn analyze_sentence(text: &str) -> Vec<ProcToken> {
    match PARSER.parse(text) {
        Ok(tokens) => filter(&tokens),
        Err(_) => Vec::new(),
    }
}

pub fn grammar() {
    println!("Pick:\n 1. Saved Text\n 2. Input Text");
    let mut inp = String::new();
    std::io::stdin().read_line(&mut inp).unwrap();
    let mut choice: String;
    match inp.trim() {
        "1" => choice = "私は毎朝早く起きて、新鮮なコーヒーを飲みながら、窓から見える庭の美しい景色を眺めるのが好きです。".into(),
        "2" => {
            println!("Input your text:");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            choice = input.trim().into();
        },
        _ => {
            println!("Invalid option. Exiting.");
            return;
        }
    }
    let mut result = PARSER.parse(&choice).unwrap();
    let menu = "Pick:\n1. Raw Japanese\n2. Raw English\n3. Filtered English\n4. New Text\n5. Exit";
    loop {
        println!("{}", menu);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        match input.trim() {
            "1" => {
                result.iter().for_each(|t| {
                    println!("{:?}", t.detail);
                });
            }
            "2" => {
                result.iter().for_each(|t| {
                    println!(
                        "{}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}",
                        t.surface, t.pos, t.sub1, t.sub2, t.sub3, t.ctype, t.cform, t.base
                    );
                });
            }
            "3" => {
                let filtered = filter(&result);
                filtered.iter().for_each(|f| {
                    println!(
                        "Word: {}, Base: {}, POS: {:?}, Conjugation: {}",
                        f.full, f.base, f.pos, f.verbPrint()
                    );
                });
            }
            "4" => {
                println!("\nInput your text:");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                choice = input.trim().into();
                result = PARSER.parse(&choice).unwrap();
            }
            "5" => break,
            _ => println!("\nInvalid option. Please try again.\n"),
        }
    }
}
