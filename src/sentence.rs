use crate::grammar::{analyze_sentence, ProcToken, PartOfSpeech, PartOfSpeechSubcategory1};

pub struct Sentence {
    pub clauses: Vec<Clause>,
}

pub struct Clause {
    pub chunks: Vec<Chunk>,
    pub relation: ClauseRelation,
}

pub struct Chunk {
    pub word: ProcToken,
    pub particle_role: Option<ParticleRole>,
    pub modifiers: Vec<Modifier>,
}

pub enum Modifier {
    Adjective(ProcToken),
    Clause(Box<Clause>),
}

#[derive(Debug)]
pub enum ClauseRelation {
    Reason,       // から、ので
    Contrast,     // けど、が
    Concessive,   // のに
    Conditional,  // ば、たら
    Continuation, // て
    Main,         // sentence-final
}

#[derive(Debug)]
pub enum ParticleRole {
    Subject,          // が
    Object,           // を
    Topic,            // は
    IndirectObject,   // に
    Destination,      // に、へ
    LocationAction,   // で
    Means,            // で
    Source,           // から (ambiguous with Reason)
    Reason,           // から、ので (ambiguous with Source)
    Limit,            // まで
}

fn build_sentence(tokens: Vec<ProcToken>) -> Option<Sentence> {
    let mut clauses: Vec<Clause> = Vec::new();
    let mut chunks: Vec<Chunk> = Vec::new();
    
    let mut i = 0;
    while i < tokens.len() {
        let current = &tokens[i];
        
        chunks.push(Chunk {
            word: current.clone(),
            particle_role: None,
            modifiers: Vec::new(),
        });

        let next = tokens.get(i + 1).map(|t| t.full.as_str());
        
        match current {
            // === CLAUSE SEPARATION ===

            // te-form verb marks continuation
            // りんごを食べて水を飲んだ
            ProcToken { pos: PartOfSpeech::Verb, full, .. } if (full.ends_with("て") || full.ends_with("で")) && next != Some("は") && next != Some("も") => {
                clauses.push(Clause { chunks, relation: ClauseRelation::Continuation });
                chunks = Vec::new();
                i += 1;
            }

            // Standalone conjunctive particles
            // 雨が降っているので行きません。
            ProcToken { pos: PartOfSpeech::Particle, sub1: PartOfSpeechSubcategory1::ConjuctiveParticle, full, .. } => {
                let relation = match full.as_str() {
                    "から" | "ので" => ClauseRelation::Reason,
                    "けど" | "が" => ClauseRelation::Contrast,
                    "のに" => ClauseRelation::Concessive,
                    "ば" | "たら" => ClauseRelation::Conditional,
                    _ => ClauseRelation::Main,
                };
                clauses.push(Clause { chunks, relation });
                chunks = Vec::new();
                i += 1;
            }

            _ => { 
                i += 1; 
            }
        }
    }

    if !chunks.is_empty() {
        clauses.push(Clause {
            chunks: chunks,
            relation: ClauseRelation::Main,
        });
    }

    Some(Sentence { clauses })
}

impl Sentence {
    pub fn print(&self) {
        println!("Sentence");
        for clause in &self.clauses {
            println!("└── Clause ({:?})", clause.relation);
            for chunk in &clause.chunks {
                print!("    ├── Chunk: {}", chunk.word.full);
                if let Some(role) = &chunk.particle_role {
                    print!(" [{:?}]", role);
                }
                println!();
                
                for modifier in &chunk.modifiers {
                    match modifier {
                        Modifier::Adjective(adj) => println!("    │   └── mod: {}", adj.full),
                        Modifier::Clause(_) => println!("    │   └── mod: [Nested Clause]"),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::analyze_sentence;

    #[test]
    fn test() {
        let tokens = analyze_sentence("雨が降っているので行きません。");
        let sentence = build_sentence(tokens).unwrap();
        sentence.print();
    }
}