use lindera::LinderaResult;
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use phf::phf_map;
use std::sync::LazyLock;
use crate::labels::{C_FORM_MAP, C_TYPE_MAP, CForms, CTypes::{self, RuVerb}, POS_MAP, POS_SUB1_MAP, POS_SUB2_MAP, POS_SUB3_MAP, PartOfSpeech, PartOfSpeechSubcategory1::{self, Unbound}, PartOfSpeechSubcategory2, PartOfSpeechSubcategory3};

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
    pub teform: bool,
    pub desiderative: bool,
    pub volitional: bool,
    pub potential: bool,
    pub causative: bool,
}

#[derive(Debug, Clone)]
pub struct ProcToken {
    pub full: String,
    pub base: String,
    pub pos: PartOfSpeech,
    pub sub1: PartOfSpeechSubcategory1,
    pub sub2: PartOfSpeechSubcategory2,
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
                if conj.teform {
                    parts.push("teform");
                }
                if conj.desiderative {
                    parts.push("desiderative");
                }
                if conj.volitional {
                    parts.push("volitional");
                }
                if conj.potential {
                    parts.push("potential");
                }
                if conj.causative {
                    parts.push("causative");
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
        let sub2 = token.sub2;
        let mut conj = token.surface.clone();

        //conjugation detection
        let mut negative=false;
        let mut past=false;
        let mut continuous=false;
        let mut teform=false;
        let mut desiderative=false;
        let mut volitional=false;
        let mut potential=false;
        let mut causative=false;
        // if token.cform != CForms::Imperfective && token.sub1 != PartOfSpeechSubcategory1::Suffix && token.ctype==CTypes::RuVerb{
        //     if line[i].base.as_str().ends_with("られる") || line[i].base.as_str().ends_with("せる") || line[i].base.as_str().ends_with("れる") || line[i].base.as_str().ends_with("ける") || line[i].base.as_str().ends_with("てる") || line[i].base.as_str().ends_with("へる") || line[i].base.as_str().ends_with("める") || line[i].base.as_str().ends_with("ねる") || line[i].base.as_str().ends_with("できる") || line[i].base.as_str().ends_with("べる") || line[i].base.as_str().ends_with("える"){potential = true};
        // }
        if(token.base=="できる"){
            potential=true;
        }
        if token.cform == CForms::Continuative || token.cform == CForms::Imperfective || token.cform == CForms::AUConnection {
            let mut j=i;
            j+=1;
            while j<line.len() && (line[j].sub1==PartOfSpeechSubcategory1::ConjuctiveParticle || line[j].cform==CForms::Continuative || line[j].cform==CForms::Imperfective){
                teform|= line[j].sub1==PartOfSpeechSubcategory1::ConjuctiveParticle;
                negative |= line[j].ctype == CTypes::IrregularNai;
                desiderative |= line[j].ctype == CTypes::IrregularTai;
                volitional |= line[j].ctype == CTypes::Invariable;
                if line[j].base.as_str().ends_with("させる") || line[j].base.ends_with("せる") {
                    causative=true;
                }
                else if line[j].base.as_str().ends_with("られる") || line[j].base.as_str().ends_with("せる") || line[j].base.as_str().ends_with("れる") || line[j].base.as_str().ends_with("ける") || line[j].base.as_str().ends_with("てる") || line[j].base.as_str().ends_with("へる") || line[j].base.as_str().ends_with("める") || line[j].base.as_str().ends_with("ねる") || line[j].base.as_str().ends_with("できる") || line[j].base.as_str().ends_with("べる") || line[j].base.as_str().ends_with("える") {potential = true};
                if j<line.len(){
                    if line[j+1].sub1 == PartOfSpeechSubcategory1::Unbound { break; } //detect new word after te
                }
                j+=1;
            }
            if j<line.len(){ //look at last token to see if conjugation
                if(teform && line[j].sub1!=PartOfSpeechSubcategory1::ConjuctiveParticle) {continuous=true};
                past |= line[j].ctype == CTypes::IrregularTa;
                negative |= line[j].ctype == CTypes::IrregularNai;
                desiderative |= line[j].ctype == CTypes::IrregularTai;
                volitional |= line[j].ctype == CTypes::Invariable;
                if line[j].base.as_str().ends_with("させる") || line[j].base.ends_with("せる") {
                    causative=true;
                }
                else if line[j].base.as_str().ends_with("られる") || line[j].base.as_str().ends_with("せる") || line[j].base.as_str().ends_with("れる") || line[j].base.as_str().ends_with("ける") || line[j].base.as_str().ends_with("てる") || line[j].base.as_str().ends_with("へる") || line[j].base.as_str().ends_with("める") || line[j].base.as_str().ends_with("ねる") || line[j].base.as_str().ends_with("できる") || line[j].base.as_str().ends_with("べる") || line[j].base.as_str().ends_with("える") {potential = true};           
            }
        }
        //conjugation detection

        let mut should_merge = false;
        if token.surface != token.base && (token.surface != "な" && token.pos != PartOfSpeech::AuxiliaryVerb) {
            should_merge = true;
        } else if i + 1 < line.len() {
            let next_token = &line[i + 1];
            if next_token.pos == PartOfSpeech::AuxiliaryVerb
               || (next_token.sub1 == PartOfSpeechSubcategory1::AdverbialParticle
                   && (next_token.surface == "じゃ" || next_token.surface == "では"))
            {
                should_merge = true;
            }
        }
        
        if should_merge {
            let mut j = i + 1;
            while j < line.len()
                && (line[j].pos == PartOfSpeech::AuxiliaryVerb
                    || line[j].sub1 == PartOfSpeechSubcategory1::Suffix
                    || (line[j].sub1 == PartOfSpeechSubcategory1::ConjuctiveParticle
                        && (line[j].surface == "て" || line[j].surface == "で"))
                    || (line[j].sub1 == PartOfSpeechSubcategory1::AdverbialParticle
                        && (line[j].surface == "じゃ" || line[j].surface == "では")))
            {
                conj = conj + &line[j].surface;
                j += 1;
            }
            i = j - 1;
        }
        filtered_tokens.push(ProcToken {
            full: conj,
            base,
            pos: pos,
            sub1,
            sub2,
            conjugation: Some(ConjugationFeatures {
                negative,
                past,
                continuous,
                teform,
                desiderative,
                volitional,
                potential,
                causative,
            })
        });
        i += 1;
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
        //"1" => choice = "私は毎朝早く起きて、新鮮なコーヒーを飲みながら、窓から見える庭の美しい景色を眺めるのが好きです。".into(),
        "1" => choice = "食べる 食べた 食べている 食べていた 食べたい 食べたくない 食べよう 食べられる 食べさせる 食べるな 食べて 食べながら 食べない 食べなかった 食べていない 食べていなかった 食べたくなかった 食べようとしない 食べられない 食べさせない".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Dumps raw Lindera tokenization for a batch of sentences.
    /// Run with: cargo test --lib grammar::tests::lindera_raw -- --nocapture
    #[test]
    fn lindera_raw() {
        let sentences = vec![
            "14つ", "十四つ",
            "12月", "十二月",
            "14ヶ月", "14か月",
        ];

        for sentence in sentences {
            println!("\n=== {} ===", sentence);
            let tokens = PARSER.parse(sentence).unwrap();
            for t in &tokens {
                println!(
                    "  {}, {:?}, {:?}, {:?}, {:?}, {:?}",
                    t.surface, t.pos, t.sub1, t.sub2, t.sub3, t.base
                );
            }
        }
    }

    /// Dumps filtered ProcToken output for a batch of sentences.
    /// Run with: cargo test --lib grammar::tests::filtered -- --nocapture
    #[test]
    fn filtered() {
        let sentences = vec![
            "もっと早く起きればよかったのにな",
        ];

        for sentence in sentences {
            println!("\n=== {} ===", sentence);
            let tokens = analyze_sentence(sentence);
            for t in &tokens {
                println!(
                    "  {}, base={}, pos={:?}, sub1={:?}, sub2={:?}",
                    t.full, t.base, t.pos, t.sub1, t.sub2
                );
            }
        }
    }
}
