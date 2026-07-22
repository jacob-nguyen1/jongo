use phf::phf_map;
use std::sync::LazyLock;
use strum_macros::AsRefStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr)]
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



pub static POS_MAP: phf::Map<&'static str, PartOfSpeech> = phf_map! {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr)]
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

pub static POS_SUB1_MAP: phf::Map<&'static str, PartOfSpeechSubcategory1> = phf_map! {
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

#[derive(Debug, Clone, Copy, AsRefStr, PartialEq)]
pub enum PartOfSpeechSubcategory2 {
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

pub static POS_SUB2_MAP: phf::Map<&'static str, PartOfSpeechSubcategory2> = phf_map! {
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

#[derive(Debug, Clone, Copy, AsRefStr)]
pub enum PartOfSpeechSubcategory3 {
    General,
    Surname,
    Name,
    Country,
    X,
    ERR,
}

pub static POS_SUB3_MAP: phf::Map<&'static str, PartOfSpeechSubcategory3> = phf_map! {
    "一般" => PartOfSpeechSubcategory3::General,
    "姓" => PartOfSpeechSubcategory3::Surname,
    "名" => PartOfSpeechSubcategory3::Name,
    "国" => PartOfSpeechSubcategory3::Country,
    "*" => PartOfSpeechSubcategory3::X,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr)]
pub enum CTypes {
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
    SuVerb,
    TsuUVerb,
    KuIVerb,
    IrregularTa,
    IrregularTai,
    IrregularNu,
    IrregularMaSu,
    IrregularNai,
    IrregularDa,
    ClassicalKi,
    ClassicalBeShi,
    ClassicalRu,
    SpecialIKu,
    X,
    ERR,
}

pub static C_TYPE_MAP: phf::Map<&'static str, CTypes> = phf_map! {
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
    "五段・サ行" => CTypes::SuVerb,
    "五段・タ行" => CTypes::TsuUVerb,
    "五段・カ行イ音便" => CTypes::KuIVerb,
    "特殊・タ" => CTypes::IrregularTa,
    "特殊・タイ" => CTypes::IrregularTai,
    "特殊・ヌ" => CTypes::IrregularNu,
    "特殊・マス" => CTypes::IrregularMaSu,
    "特殊・ナイ" => CTypes::IrregularNai,
    "特殊・ダ" => CTypes::IrregularDa,
    "文語・キ" => CTypes::ClassicalKi,
    "文語・ベシ" => CTypes::ClassicalBeShi,
    "文語・ル" => CTypes::ClassicalRu,
    "五段・カ行促音便" => CTypes::SpecialIKu,
    "*" => CTypes::X,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr)]
pub enum CForms {
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
    Conditional,
    X,
    ERR,
}

pub static C_FORM_MAP: phf::Map<&'static str, CForms> = phf_map! {
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
    "連用テ接続" => CForms::Continuative,
    "仮定形" => CForms::Conditional,
    "*" => CForms::X,
};

//sentence.rs
#[derive(Debug, Clone)]
pub enum ClauseRelation {
    Reason,       // から、ので (ConjunctiveParticle)
    Contrast,     // けど、が (ConjunctiveParticle)
    Concessive,   // のに
    Conditional,  // ば、たら
    Continuation, // て
    Sequence,     // てから
    Simultaneous, // ながら
    Temporal,     // 時 (when)
    Until,        // まで after verb
    Main,         // sentence-final
    Modifier,     // relative clause
    Quotation,    // と after verb
    Evidential,   // によると (according to)
    Ambiguous(Vec<ClauseRelation>), // When rule-based parsing cannot distinguish (e.g., conditional vs quotation と)
}

impl ClauseRelation {
    pub fn all() -> &'static [ClauseRelation] {
        &[
            Self::Reason,
            Self::Contrast,
            Self::Concessive,
            Self::Conditional,
            Self::Continuation,
            Self::Sequence,
            Self::Simultaneous,
            Self::Temporal,
            Self::Until,
            Self::Main,
            Self::Modifier,
            Self::Quotation,
            Self::Evidential,
        ]
    }

    pub fn explanation(&self) -> &'static str {
        match self {
            Self::Reason => "Gives a reason or cause (から、ので).",
            Self::Contrast => "Contrasts with the following clause (けど、が).",
            Self::Concessive => "Unexpected outcome despite the clause (のに).",
            Self::Conditional => "Sets a condition (ば、たら).",
            Self::Continuation => "Continues into the next action (て).",
            Self::Sequence => "One action after another (てから).",
            Self::Simultaneous => "Two actions happening at once (ながら).",
            Self::Temporal => "Indicates when something happens (時).",
            Self::Until => "Marks an endpoint in time or space (まで).",
            Self::Main => "The main / sentence-final clause.",
            Self::Modifier => "A relative clause modifying a noun.",
            Self::Quotation => "Quotes speech or thought (と).",
            Self::Evidential => "Indicates source of information (によると).",
            Self::Ambiguous(_) => "Cannot be determined by rule-based parsing alone.",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::Reason => "#a040a0",
            Self::Contrast | Self::Concessive => "#e07000",
            Self::Conditional => "#40a040",
            Self::Continuation | Self::Sequence | Self::Simultaneous => "#008080",
            Self::Main => "#000000",
            Self::Modifier => "#666666",
            Self::Quotation | Self::Evidential => "#7070a0",
            Self::Temporal | Self::Until => "#88aa44",
            Self::Ambiguous(_) => "#888888",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Reason => "Reason",
            Self::Contrast => "Contrast",
            Self::Concessive => "Concessive",
            Self::Conditional => "Conditional",
            Self::Continuation => "Continuation",
            Self::Sequence => "Sequence",
            Self::Simultaneous => "Simultaneous",
            Self::Temporal => "Temporal",
            Self::Until => "Until",
            Self::Main => "Main",
            Self::Modifier => "Modifier",
            Self::Quotation => "Quotation",
            Self::Evidential => "Evidential",
            Self::Ambiguous(_) => "Ambiguous",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticleRole {
    Subject,          // が
    Object,           // を
    Topic,            // は
    IndirectObject,   // に (default)
    Destination,      // に、へ
    LocationAction,   // で (default)
    Means,            // で
    Source,           // から
    Limit,            // まで
    Also,             // も
    ComparisonBase,   // より
    Accompaniment,    // と
    Listing,          // や、と
    Temporal,         // に
    Purpose,          // に
    Agent,            // に
    Adverbial,        // に
    Scope,            // で
    Approximate,      // ぐらい
    Definition,       // とは
    Ambiguous(Vec<ParticleRole>), // unresolved candidates, for LLM resolution later
}

impl ParticleRole {
    pub fn all() -> &'static [ParticleRole] {
        &[
            Self::Subject,
            Self::Object,
            Self::Topic,
            Self::IndirectObject,
            Self::Destination,
            Self::LocationAction,
            Self::Means,
            Self::Source,
            Self::Limit,
            Self::Also,
            Self::ComparisonBase,
            Self::Accompaniment,
            Self::Listing,
            Self::Temporal,
            Self::Purpose,
            Self::Agent,
            Self::Adverbial,
            Self::Scope,
            Self::Approximate,
            Self::Definition,
        ]
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Subject => "Subject",
            Self::Object => "Object",
            Self::Topic => "Topic",
            Self::IndirectObject => "Indirect Object",
            Self::Destination => "Destination",
            Self::LocationAction => "Action Location",
            Self::Means => "Means/Method",
            Self::Source => "Source",
            Self::Limit => "Limit",
            Self::Also => "Also",
            Self::ComparisonBase => "Comparison Base",
            Self::Accompaniment => "Accompaniment",
            Self::Listing => "Listing",
            Self::Temporal => "Time",
            Self::Purpose => "Purpose",
            Self::Agent => "Agent (Passive/Causative)",
            Self::Adverbial => "Adverbial",
            Self::Scope => "Scope",
            Self::Approximate => "Approximate",
            Self::Definition => "Definition",
            Self::Ambiguous(_) => "Ambiguous",
        }
    }

    pub fn explanation(&self) -> &'static str {
        match self {
            Self::Subject => "Performs the action or is described.",
            Self::Object => "Direct receiver of the action.",
            Self::Topic => "The main topic of the sentence.",
            Self::IndirectObject => "Receiver of the action.",
            Self::Destination => "Where the action is heading.",
            Self::LocationAction => "Where the action takes place.",
            Self::Means => "How the action is done.",
            Self::Source => "Where the action started.",
            Self::Limit => "The limit of the action in time or space.",
            Self::Also => "Includes this item as well.",
            Self::ComparisonBase => "The baseline for a comparison.",
            Self::Accompaniment => "Who the action is done with.",
            Self::Listing => "An incomplete list of items.",
            Self::Temporal => "When the action happens.",
            Self::Purpose => "Why the action is happening.",
            Self::Agent => "Who performs the action in passive/causative.",
            Self::Adverbial => "Turns the word into an adverb.",
            Self::Scope => "The scope or boundary of the action.",
            Self::Approximate => "An approximate amount or point.",
            Self::Definition => "Defines or explains the preceding term.",
            Self::Ambiguous(_) => "Cannot be determined by rule-based parsing alone.",
        }
    }

    /// Parse a string (from LLM response) back into a ParticleRole.
    /// Accepts both Debug names ("IndirectObject") and badge names ("Indirect Object").
    pub fn from_str(s: &str) -> Option<ParticleRole> {
        match s {
            "Subject" => Some(Self::Subject),
            "Object" => Some(Self::Object),
            "Topic" => Some(Self::Topic),
            "IndirectObject" | "Indirect Object" => Some(Self::IndirectObject),
            "Destination" => Some(Self::Destination),
            "LocationAction" | "Action Location" => Some(Self::LocationAction),
            "Means" | "Means/Method" => Some(Self::Means),
            "Source" => Some(Self::Source),
            "Limit" => Some(Self::Limit),
            "Also" => Some(Self::Also),
            "ComparisonBase" | "Comparison Base" => Some(Self::ComparisonBase),
            "Accompaniment" => Some(Self::Accompaniment),
            "Listing" => Some(Self::Listing),
            "Temporal" | "Time" => Some(Self::Temporal),
            "Purpose" => Some(Self::Purpose),
            "Agent" | "Agent (Passive/Causative)" => Some(Self::Agent),
            "Adverbial" => Some(Self::Adverbial),
            "Scope" => Some(Self::Scope),
            "Approximate" => Some(Self::Approximate),
            "Definition" => Some(Self::Definition),
            _ => None,
        }
    }
}