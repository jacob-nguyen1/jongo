use crate::labels::{
    C_FORM_MAP, C_TYPE_MAP, CForms, CTypes, POS_MAP, POS_SUB1_MAP, POS_SUB2_MAP, POS_SUB3_MAP,
    PartOfSpeech, PartOfSpeechSubcategory1, PartOfSpeechSubcategory2, PartOfSpeechSubcategory3,
};
use lindera::LinderaResult;
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use std::sync::LazyLock;

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
    pub conditional: bool,
    pub negimperative: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaircaseStep {
    pub text: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ProcToken {
    pub full: String,
    pub base: String,
    pub pos: PartOfSpeech,
    pub sub1: PartOfSpeechSubcategory1,
    pub sub2: PartOfSpeechSubcategory2,
    pub conjugation: Option<ConjugationFeatures>,
    pub staircase: Option<Vec<StaircaseStep>>,
    pub reading: Option<String>,
}

impl ProcToken{
    pub fn verb_print(&self) -> Option<String> {
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
                if conj.conditional {
                    parts.push("conditional");
                }
                if conj.negimperative {
                    parts.push("negimperative");
                }
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join(", "))
                }
            }
            None => None,
        }
    }
}

fn filter(line: &[Token]) -> Vec<ProcToken> {
    let mut filtered_tokens: Vec<ProcToken> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let token = line.get(i).unwrap();
        let mut pos = token.pos.clone();
        if token.surface.chars().all(|c| matches!(c, '―' | '—' | '…' | '・' | '−' | '〜' | '～' | '‐')) {
            pos = PartOfSpeech::Symbol;
        }
        let base = token.base.clone();
        let sub1 = token.sub1.clone();
        let sub2 = token.sub2;
        let mut conj = token.surface.clone();
        let reading = token.detail.get(7).filter(|s| *s != "*").cloned();

        if token.surface == "か"
            && i + 2 < line.len()
            && line[i + 1].surface == "どう"
            && line[i + 2].surface == "か"
        {
            filtered_tokens.push(ProcToken {
                full: "かどうか".to_string(),
                base: "かどうか".to_string(),
                pos: PartOfSpeech::Noun,
                sub1: PartOfSpeechSubcategory1::Bound,
                sub2: PartOfSpeechSubcategory2::X,
                conjugation: None,
                staircase: None,
                reading: Some("カドウカ".to_string()),
            });
            i += 3;
            continue;
        }

        //conjugation detection

        let mut state: ConjugationFeatures = ConjugationFeatures {
            negative: false,
            past: false,
            continuous: false,
            teform: false,
            desiderative: false,
            volitional: false,
            potential: false,
            causative: false,
            conditional: false,
            negimperative: false,
        };

        // let mut negative=false;
        // let mut past=false;
        // let mut continuous=false;
        // let mut teform=false;
        // let mut desiderative=false;
        // let mut volitional=false;
        // let mut potential=false;
        // let mut causative=false;
        // //let mut conditional = token.cform == CForms::Conditional;
        // let mut conditional=false;
        // let mut negimperative=false;
        // if token.cform != CForms::Imperfective && token.sub1 != PartOfSpeechSubcategory1::Suffix && token.ctype==CTypes::RuVerb{
        //     if line[i].base.as_str().ends_with("られる") || line[i].base.as_str().ends_with("せる") || line[i].base.as_str().ends_with("れる") || line[i].base.as_str().ends_with("ける") || line[i].base.as_str().ends_with("てる") || line[i].base.as_str().ends_with("へる") || line[i].base.as_str().ends_with("める") || line[i].base.as_str().ends_with("ねる") || line[i].base.as_str().ends_with("できる") || line[i].base.as_str().ends_with("べる") || line[i].base.as_str().ends_with("える"){potential = true};
        // }
        if token.base == "できる" {
            state.potential = true;
        }

        // Detect potential form for non-Ru verbs like 貸せる, 飛べる, 洗える.
        // Lindera parses these as standard `RuVerb`. We check if they end in standard potential
        // ichidan suffix patterns, AND are NOT native JMDict verbs (like 食べる, 見せる).
        if token.pos == PartOfSpeech::Verb && token.ctype == CTypes::RuVerb {
            let base = token.base.as_str();
            if base.ends_with("ける") || base.ends_with("げる") || base.ends_with("せる") || 
               base.ends_with("てる") || base.ends_with("ねる") || base.ends_with("べる") || 
               base.ends_with("める") || base.ends_with("れる") || base.ends_with("える") {
                if !crate::jmdict::is_native_verb(base) {
                    state.potential = true;
                }
            }
        }

        // detect negimperative for verbs followed by "な" (e.g., 食べるな)
        if token.pos == PartOfSpeech::Verb && i + 1 < line.len() && line[i + 1].base == "な" {
            state.negimperative = true;
            conj += "な";
        }

        // conjugation detection
        let mut should_merge = false;

        let is_mergeable_auxiliary = |prev: &Token, aux: &Token| -> bool {
            if aux.pos != PartOfSpeech::AuxiliaryVerb {
                return false;
            }
            let base = aux.base.as_str();

            // 1. Purely Conjugational Auxiliaries (ALWAYS MERGE)
            if matches!(
                base,
                "ない"
                    | "ぬ"
                    | "ん"
                    | "ます"
                    | "た"
                    | "せる"
                    | "させる"
                    | "れる"
                    | "られる"
                    | "たい"
                    | "う"
                    | "よう"
            ) {
                return true;
            }
            if base == "だ"
                && (aux.ctype == CTypes::IrregularTa || aux.ctype == CTypes::IrregularDa)
            {
                return true;
            }

            // 2. Attributive Copula 'な' (NEVER MERGE)
            // We used to conditionally merge this for AdjectiveVerbStems, but we now treat 'な' as a particle in sentence.rs!

            false
        };

        if token.surface != token.base
            && (token.surface != "な" && token.pos != PartOfSpeech::AuxiliaryVerb && token.pos != PartOfSpeech::Noun)
        {
            should_merge = true;
        } else if i + 1 < line.len() {
            let next_token = &line[i + 1];
            if is_mergeable_auxiliary(&token, next_token)
                || (next_token.sub1 == PartOfSpeechSubcategory1::AdverbialParticle
                    && (next_token.surface == "じゃ" || next_token.surface == "では"))
                || (next_token.sub1 == PartOfSpeechSubcategory1::ConjuctiveParticle
                    && next_token.surface == "ば")
            {
                should_merge = true;
            }
        }

        // G5: Handle 'である' merging explicitly
        let base = if token.surface == "で"
            && token.base == "だ"
            && i + 1 < line.len()
            && line[i + 1].base == "ある"
        {
            should_merge = true;
            "である".to_string() // Override base so JMdict hits 'である'
        } else {
            base
        };

        let mut staircase: Option<Vec<StaircaseStep>> = None;
        if should_merge {
            let mut steps = Vec::new();
            steps.push(StaircaseStep {
                text: base.clone(),
                description: "Base".to_string(),
            });
            let mut cumulative_surface = token.surface.clone();

            let mut j = i + 1;
            // If negimperative is true, j should skip the "な"
            if state.negimperative {
                j += 1;
            }

            while j < line.len()
                && (is_mergeable_auxiliary(&line[j - 1], &line[j])
                    || line[j].sub1 == PartOfSpeechSubcategory1::Suffix
                    || (line[j].sub1 == PartOfSpeechSubcategory1::ConjuctiveParticle
                        && (line[j].surface == "て" || line[j].surface == "で" || line[j].surface == "ば"))
                    || (line[j].sub1 == PartOfSpeechSubcategory1::AdverbialParticle
                        && (line[j].surface == "じゃ" || line[j].surface == "では"))
                    || ((line[j].sub1 == PartOfSpeechSubcategory1::Bound
                        || line[j].sub1 == PartOfSpeechSubcategory1::DependentVerb)
                        && (line[j].base == "いる"
                            || line[j].base == "い"
                            || line[j].base == "おる"
                            || line[j].base == "おり"))
                    || (line[j - 1].surface == "で"
                        && line[j - 1].base == "だ"
                        && line[j].base == "ある")
                    || (line[j - 1].sub1 == PartOfSpeechSubcategory1::Number
                        && line[j].surface == "月"))
            {
                let next_token = &line[j];

                let desc = match next_token.base.as_str() {
                    "ます" => "Polite",
                    "た" | "だ" if next_token.surface == "たら" => {
                        state.past = true;
                        state.conditional = true;
                        "Conditional"
                    }
                    "た" | "だ" if next_token.surface == "なら" => {
                        state.conditional = true;
                        "Conditional"
                    }
                    "た" | "だ" => {
                        state.past = true;
                        "Past"
                    }
                    "ない" | "ん" | "ぬ" => {
                        state.negative = true;
                        "Negative"
                    }
                    "たい" => {
                        state.desiderative = true;
                        "Desire"
                    }
                    "られる" | "できる" => {
                        state.potential = true;
                        "Potential / Passive"
                    }
                    "れる" => {
                        state.potential = true;
                        "Passive"
                    }
                    "せる" | "させる" => {
                        state.causative = true;
                        "Causative"
                    }
                    "て" | "で" => {
                        state.teform = true;
                        "Te-Form"
                    }
                    "いる" | "おる" => {
                        state.continuous = true;
                        state.teform = false;
                        "Continuous"
                    }
                    "ば" | "なら" => {
                        state.conditional = true;
                        "Conditional"
                    }
                    "う" | "よう" => {
                        state.volitional = true;
                        "Volitional"
                    }
                    _ => "Auxiliary",
                };

                steps.push(StaircaseStep {
                    text: format!("{}{}", cumulative_surface, next_token.base),
                    description: desc.to_string(),
                });

                cumulative_surface.push_str(&next_token.surface);
                conj = conj + &next_token.surface;
                j += 1;
            }
            if steps.len() > 1 {
                staircase = Some(steps);
            }
            i = j - 1;
        } else if state.negimperative {
            i += 1;
        } else if pos == PartOfSpeech::Noun {
            let mut j = i + 1;
            while j < line.len() {
                let nxt = &line[j];
                if (sub1 == PartOfSpeechSubcategory1::Number
                    && ((nxt.pos == PartOfSpeech::Noun
                        && (nxt.sub1 == PartOfSpeechSubcategory1::Number
                            || nxt.surface == "月"
                            || nxt.surface == "目"))
                        || (nxt.pos == PartOfSpeech::AuxiliaryVerb && nxt.surface == "つ")))
                    || nxt.sub1 == PartOfSpeechSubcategory1::Suffix
                {
                    conj = conj + &nxt.surface;
                    j += 1;
                } else {
                    break;
                }
            }
            i = j - 1;
        }
        filtered_tokens.push(ProcToken {
            full: conj,
            base: base.clone(),
            pos: pos,
            sub1,
            sub2,
            conjugation: Some(state),
            staircase,
            reading,
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
        
        // Load custom user dictionary compiled from custom_dict.csv at build time
        let custom_dict_bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/custom_dict.bin"));
        let user_dict = if !custom_dict_bytes.is_empty() {
            use lindera::dictionary::UserDictionary;
            Some(UserDictionary::load(custom_dict_bytes).expect("failed to load custom_dict.bin"))
        } else {
            None
        };
        
        let segmenter = Segmenter::new(Mode::Normal, dictionary, user_dict);
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

                macro_rules! map_detail {
                    ($idx:expr, $map:expr, $err:expr) => {
                        details
                            .get($idx)
                            .and_then(|k| $map.get(*k))
                            .copied()
                            .unwrap_or($err)
                    };
                }

                macro_rules! detail_or_surface {
                    ($idx:expr, $surface:expr) => {
                        details
                            .get($idx)
                            .map(|s| (*s).to_string())
                            .unwrap_or($surface.clone())
                    };
                }

                let pos = map_detail!(0, POS_MAP, PartOfSpeech::ERR);
                let sub1 = map_detail!(1, POS_SUB1_MAP, PartOfSpeechSubcategory1::ERR);
                let sub2 = map_detail!(2, POS_SUB2_MAP, PartOfSpeechSubcategory2::ERR);
                let sub3 = map_detail!(3, POS_SUB3_MAP, PartOfSpeechSubcategory3::ERR);
                let ctype = map_detail!(4, C_TYPE_MAP, CTypes::ERR);
                let cform = map_detail!(5, C_FORM_MAP, CForms::ERR);
                let base = detail_or_surface!(6, surface);

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
            .collect::<Vec<Token>>();

        Ok(parsed_tokens)
    }
}

pub fn remove_parentheses(text: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    for c in text.chars() {
        if c == '（' || c == '(' {
            depth += 1;
        } else if c == '）' || c == ')' {
            if depth > 0 {
                depth -= 1;
            }
        } else if depth == 0 {
            if !c.is_whitespace() {
                result.push(c);
            }
        }
    }
    result
}

pub fn analyze_sentence(text: &str) -> Vec<ProcToken> {
    let clean_text = remove_parentheses(text);
    match PARSER.parse(&clean_text) {
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
        "1" => choice = "食べる 食べた 食べている 食べていた 食べたい 食べたくない 食べよう 食べられる 食べさせる 食べるな 食べて 食べながら 食べない 食べなかった 食べていない 食べていなかった 食べたくなかった 食べようとしない 食べられない 食べさせない 食べたら 食べるば 食べるなら".into(),
        //"1" => choice = "7月15日に友人と映画館へ行きました。".into(),
        //"1" => choice = "指させる 指せる 貸させる 貸せる 押させる 押せる 出させる 出せる 返させる 返せる 探させる 探せる 洗わせる 洗える 助けさせる 助けられる 鍛えさせる 鍛えられる 増させる 増せる 変えさせる 変えられる 考えさせる 考えられる 走らせる 走れる 飛ばさせる 飛べる 乗らせる 乗れる 売らせる 売れる 切らせる 切れる 来させる 来られる できさせる できる".into(),
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
                        f.full, f.base, f.pos, f.verb_print().unwrap_or("none".to_string())
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
            "たべたら", "指させる", "指せる"
        ];
        
        for sentence in sentences {
            println!("\n=== {} ===", sentence);
            let tokens = PARSER.parse(sentence).unwrap();
            for t in &tokens {
                println!(
                    "  {}, ctype={:?}, cform={:?}, detail={:?}",
                    t.surface, t.ctype, t.cform, t.detail
                );
            }
        }
    }

    /// Dumps filtered ProcToken output for a batch of sentences.
    /// Run with: cargo test --lib grammar::tests::filtered -- --nocapture
    #[test]
    fn filtered() {
        let sentences = vec![
            "たべたら", "指させる", "指せる", "貸させる", "貸せる", "押させる", "押せる", "出させる", "出せる",
            "返させる", "返せる", "探させる", "探せる", "洗わせる", "洗える", "助けさせる", "助けられる",
            "鍛えさせる", "鍛えられる", "増させる", "増せる", "変えさせる", "変えられる", "考えさせる", "考えられる",
            "走らせる", "走れる", "飛ばさせる", "飛べる", "乗らせる", "乗れる", "売らせる", "売れる", "切らせる", "切れる", "来させる", "来られる", "できさせる", "できる"
        ];

        for sentence in sentences {
            println!("\n=== {} ===", sentence);
            let tokens = analyze_sentence(sentence);
            for t in &tokens {
                println!(
                    "  {}, base={}, pos={:?}, sub1={:?}, sub2={:?}, conj={:?}",
                    t.full, t.base, t.pos, t.sub1, t.sub2, t.verb_print()
                );
            }
        }

        // Verify 食べたら is detected as conditional
        let tokens = analyze_sentence("食べたら");
        assert_eq!(tokens.len(), 1);
        let conj = tokens[0].conjugation.as_ref().unwrap();
        assert!(conj.conditional, "食べたら should be conditional");
        assert!(conj.past, "食べたら should be past");
    }
}
