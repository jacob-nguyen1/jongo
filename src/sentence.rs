use crate::grammar::{analyze_sentence, ProcToken, PartOfSpeech};

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
    let mut iter = tokens.into_iter().peekable();

    while let Some(token) = iter.next() {
        let next = iter.peek();
        
        chunks.push(Chunk {
            word: token,
            particle_role: None,
            modifiers: Vec::new(),
        })
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
        let tokens = analyze_sentence("りんごを食べた人は走っている");
        let sentence = build_sentence(tokens).unwrap();
        sentence.print();
    }
}