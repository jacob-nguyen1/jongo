use crate::grammar::{analyze_sentence, ProcToken, PartOfSpeech, PartOfSpeechSubcategory1};

pub struct Sentence {
    pub clauses: Vec<Clause>,
}

pub struct Clause {
    pub chunks: Vec<Chunk>,
    pub relation: ClauseRelation,
    pub connective: Option<ProcToken>,
}

impl Clause {
    pub fn text(&self) -> String {
        let mut text: String = self.chunks.iter().filter_map(|c| {
            if c.word.pos == PartOfSpeech::Symbol {
                None
            } else {
                let mut s = c.word.full.clone();
                if let Some(p) = &c.particle {
                    s.push_str(&p.full);
                }
                Some(s)
            }
        }).collect();
        
        if let Some(conn) = &self.connective {
            text.push_str(&conn.full);
        }
        
        text
    }
}

pub struct Chunk {
    pub word: ProcToken,
    pub particle: Option<ProcToken>,
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
    Modifier,     // Relative clauses
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
    let mut pending_modifiers: Vec<Modifier> = Vec::new();
    
    let mut i = 0;
    while i < tokens.len() {
        let current = &tokens[i];
        
        chunks.push(Chunk {
            word: current.clone(),
            particle: None,
            particle_role: None,
            modifiers: std::mem::take(&mut pending_modifiers),
        });

        let next_token = tokens.get(i + 1);
        let next_str = next_token.map(|t| t.full.as_str());
        let next_pos = next_token.map(|t| t.pos);
        
        match current {
            // === MODIFIER DETECTION ===
            
            // Relative clause: Verb immediately followed by Noun
            ProcToken { pos: PartOfSpeech::Verb, .. } if next_pos == Some(PartOfSpeech::Noun) => {
                let modifier_clause = Clause { chunks, relation: ClauseRelation::Modifier, connective: None };
                pending_modifiers.push(Modifier::Clause(Box::new(modifier_clause)));
                chunks = Vec::new();
                i += 1;
            }

            // === CLAUSE SEPARATION ===

            // te-form verb marks continuation
            // りんごを食べて水を飲んだ
            ProcToken { pos: PartOfSpeech::Verb, full, .. } if (full.ends_with("て") || full.ends_with("で")) && next_str != Some("は") && next_str != Some("も") => {
                clauses.push(Clause { chunks, relation: ClauseRelation::Continuation, connective: None });
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
                let connective = chunks.pop().map(|c| c.word);
                clauses.push(Clause { chunks, relation, connective });
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
            connective: None,
        });
    }

    let mut sentence = Sentence { clauses };

    // ==========================================
    // PASS 2: PARTICLE ROLE ASSIGNMENT
    // ==========================================
    for clause in &mut sentence.clauses {
        assign_particle_roles(clause);
    }

    Some(sentence)
}

fn assign_particle_roles(clause: &mut Clause) {
    let mut new_chunks = Vec::new();
    let old_chunks = std::mem::take(&mut clause.chunks);
    let mut iter = old_chunks.into_iter().peekable();
    
    while let Some(mut chunk) = iter.next() {
        if chunk.word.pos == PartOfSpeech::Noun {
            if let Some(next_chunk) = iter.peek() {
                let next_word = &next_chunk.word;
                if next_word.pos == PartOfSpeech::Particle &&
                   (next_word.sub1 == PartOfSpeechSubcategory1::MarkingParticle || 
                    next_word.sub1 == PartOfSpeechSubcategory1::LinkingParticle) 
                {
                    chunk.particle_role = match next_word.full.as_str() {
                        "が" => Some(ParticleRole::Subject),
                        "を" => Some(ParticleRole::Object),
                        "は" => Some(ParticleRole::Topic),
                        "に" => Some(ParticleRole::IndirectObject),
                        "へ" => Some(ParticleRole::Destination),
                        "で" => Some(ParticleRole::LocationAction),
                        "から" => Some(ParticleRole::Source),
                        "まで" => Some(ParticleRole::Limit),
                        _ => None, // Unmapped particle
                    };
                    chunk.particle = Some(next_word.clone());
                    
                    // Consume the particle so it doesn't become a standalone chunk
                    iter.next();
                }
            }
        }

        // recurse into modifier clauses
        for modifier in &mut chunk.modifiers {
            if let Modifier::Clause(inner_clause) = modifier {
                assign_particle_roles(inner_clause);
            }
        }

        new_chunks.push(chunk);
    }
    clause.chunks = new_chunks;
}

impl Sentence {
    pub fn print(&self) {
        println!("Sentence");
        for clause in &self.clauses {
            println!("└── Clause ({:?})", clause.relation);
            
            for chunk in &clause.chunks {
                if chunk.word.pos == PartOfSpeech::Symbol {
                    continue;
                }
                
                print!("    ├── Chunk: {}", chunk.word.full);
                if let Some(particle) = &chunk.particle {
                    print!(" {}", particle.full);
                }
                if let Some(role) = &chunk.particle_role {
                    print!(" [{:?}]", role);
                }
                println!();
                
                for modifier in &chunk.modifiers {
                    match modifier {
                        Modifier::Adjective(adj) => println!("    │   └── mod: {}", adj.full),
                        Modifier::Clause(clause) => println!("    │   └── mod: [{}]", clause.text()),
                    }
                }
            }
            
            if let Some(conn) = &clause.connective {
                println!("    └── Connective: {}", conn.full);
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
        let tokens = analyze_sentence("彼女が作った料理を食べたけど、あまり好きじゃなかった。");
        let sentence = build_sentence(tokens).unwrap();
        sentence.print();
    }
}