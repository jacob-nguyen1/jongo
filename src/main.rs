use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use lindera::LinderaResult;

use phf::phf_map;

static POS_MAP: phf::Map<&'static str, &'static str> = phf_map! {
    "名詞" => "noun",
    "助詞" => "particle",
    "助動詞" => "auxiliary verb",
};

fn main() -> LinderaResult<()> {
    println!("{}", POS_MAP.get("名詞").unwrap());
    let dictionary = load_dictionary("embedded://ipadic")?;
    let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
    let tokenizer = Tokenizer::new(segmenter);

    let text = "私は学生だ";
    let mut tokens = tokenizer.tokenize(text)?;
    println!("text:\t{}", text);
    for token in tokens.iter_mut() {
        let details = token.details().join(",");
        println!("token:\t{}\t{}", token.surface.as_ref(), details);
    }

    Ok(())
}