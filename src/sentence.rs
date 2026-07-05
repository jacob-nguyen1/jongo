use crate::grammar::{analyze_sentence, ProcToken, PartOfSpeech, PartOfSpeechSubcategory1, PartOfSpeechSubcategory2};

pub struct Sentence {
    pub clauses: Vec<Clause>,
}

#[derive(Debug)]
pub struct Clause {
    pub chunks: Vec<Chunk>,
    pub relation: ClauseRelation,
    pub connective: Option<ProcToken>,
    pub ending_particles: Vec<ProcToken>,
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

#[derive(Debug)]
pub struct Chunk {
    pub word: ProcToken,
    pub particle: Option<ProcToken>,
    pub particle_role: Option<ParticleRole>,
    pub modifiers: Vec<Modifier>,
    pub is_head: bool,
}

#[derive(Debug)]
pub enum Modifier {
    Adjective(ProcToken),
    Limitation(Box<Chunk>),
    Clause(Box<Clause>),
}

#[derive(Debug)]
pub enum ClauseRelation {
    Reason,       // から、ので (ConjunctiveParticle)
    Contrast,     // けど、が (ConjunctiveParticle)
    Concessive,   // のに
    Conditional,  // ば、たら
    Continuation, // て
    Sequence,     // てから
    Simultaneous, // ながら
    Until,        // まで after verb
    Main,         // sentence-final
    Modifier,     // relative clause
    Quotation,    // と after verb
}

#[derive(Debug, Clone)]
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
    Temporal,         // に
    Purpose,          // に
    Agent,            // に
    Adverbial,        // に
    Ambiguous(Vec<ParticleRole>), // unresolved candidates, for LLM resolution later
}

pub fn build_sentence(tokens: Vec<ProcToken>) -> Option<Sentence> {
    let tokens: Vec<ProcToken> = tokens
        .into_iter()
        .filter(|t| t.pos != PartOfSpeech::Symbol)
        .collect();

    let mut clauses: Vec<Clause> = Vec::new();
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut pending_modifiers: Vec<Modifier> = Vec::new();
    let mut pending_ending_particles: Vec<ProcToken> = Vec::new();
    
    let mut i = 0;
    while i < tokens.len() {
        let current = &tokens[i];
        
        chunks.push(Chunk {
            word: current.clone(),
            particle: None,
            particle_role: None,
            modifiers: std::mem::take(&mut pending_modifiers),
            is_head: true,
        });

        let next_token = tokens.get(i + 1);
        let next_str = next_token.map(|t| t.full.as_str());
        let next_pos = next_token.map(|t| t.pos);
        
        match current {
            // === MODIFIER DETECTION ===
            
            // Adjective て-form Continuation split
            ProcToken { pos: PartOfSpeech::Adjective, full, .. }
                if full.ends_with("て") || full.ends_with("で") => {
                clauses.push(Clause {
                    chunks,
                    relation: ClauseRelation::Continuation,
                    connective: None,
                    ending_particles: std::mem::take(&mut pending_ending_particles),
                });
                chunks = Vec::new();
                i += 1;
            }

            // い-adjective modifier
            ProcToken { pos: PartOfSpeech::Adjective, .. } if next_pos == Some(PartOfSpeech::Noun) => {
                let mut adj_chunk = chunks.pop().unwrap();
                // Bubble up any modifiers that accidentally attached to the adjective
                let extracted = std::mem::take(&mut adj_chunk.modifiers);
                pending_modifiers.extend(extracted);
                pending_modifiers.push(Modifier::Adjective(adj_chunk.word));
                i += 1;
            }

            // な-adjective modifier (e.g. 好きな + 料理)
            // Lindera tags the stem as Noun/AdjectiveVerbStem, and grammar.rs merges the 'な' auxiliary into it.
            ProcToken { pos: PartOfSpeech::Noun, sub1: PartOfSpeechSubcategory1::AdjectiveVerbStem, .. } 
                if current.full.ends_with("な") && next_pos == Some(PartOfSpeech::Noun) => {
                // If it's the ending 'の', we shouldn't trigger the modifier rule.
                let next = tokens.get(i + 1).unwrap();
                let is_ending_no = next.full == "の" && match tokens.get(i + 2) {
                    Some(n2) => n2.sub1 == PartOfSpeechSubcategory1::EndingParticle 
                             || n2.sub2 == PartOfSpeechSubcategory2::Quotation
                             || n2.full == "に",
                    None => true,
                };
                
                if !is_ending_no {
                    let mut adj_chunk = chunks.pop().unwrap();
                    let extracted = std::mem::take(&mut adj_chunk.modifiers);
                    pending_modifiers.extend(extracted);
                    pending_modifiers.push(Modifier::Adjective(adj_chunk.word));
                }
                i += 1;
            }

            // のに concessive clause split
            ProcToken { pos: PartOfSpeech::Noun, sub1: PartOfSpeechSubcategory1::Bound, full, .. }
                if full == "の" && next_str == Some("に") => {
                let no_chunk = chunks.pop().unwrap();
                let ni_token = tokens.get(i + 1).unwrap().clone();
                let mut connective_token = no_chunk.word;
                connective_token.full = format!("{}に", connective_token.full); // combine into "のに"
                clauses.push(Clause {
                    chunks,
                    relation: ClauseRelation::Concessive,
                    connective: Some(connective_token),
                    ending_particles: std::mem::take(&mut pending_ending_particles),
                });
                chunks = Vec::new();
                i += 2;
            }

            // "の" Particle linking two nouns
            ProcToken { pos: PartOfSpeech::Particle, sub1: PartOfSpeechSubcategory1::NormalizingParticle, .. } => {
                let no_chunk = chunks.pop().unwrap(); // Remove the `の` chunk
                if let Some(prev_chunk) = chunks.pop() {
                    let mut lim_chunk = if prev_chunk.word.pos == PartOfSpeech::Particle {
                        let mut particle_token = prev_chunk.word;
                        particle_token.full.push_str(&no_chunk.word.full);
                        if let Some(mut noun_chunk) = chunks.pop() {
                            noun_chunk.particle = Some(particle_token);
                            noun_chunk
                        } else {
                            Chunk { word: particle_token, particle: None, particle_role: None, modifiers: Vec::new(), is_head: true }
                        }
                    } else {
                        let mut prev_chunk = prev_chunk;
                        prev_chunk.particle = Some(no_chunk.word);
                        prev_chunk
                    };
                    lim_chunk.is_head = false;

                    // F2: Skip non-head chunks in modifier attachment (bubble up to the real head)
                    // We currently partition: eager modifiers (Adjectives, Clauses) bubble up,
                    // but Limitation modifiers stay nested. This is a temporary structural default
                    // pending a larger architectural decision on attachment ambiguity.
                    let (eager, nested): (Vec<_>, Vec<_>) = lim_chunk.modifiers.into_iter().partition(|m| {
                        matches!(m, Modifier::Adjective(_) | Modifier::Clause(_))
                    });
                    lim_chunk.modifiers = nested;
                    pending_modifiers.extend(eager);
                    
                    pending_modifiers.push(Modifier::Limitation(Box::new(lim_chunk)));
                }
                i += 1;
            }

            // === CLAUSE SEPARATION ===

            // te-form verb marks continuation
            // りんごを食べて水を飲んだ
            ProcToken { pos: PartOfSpeech::Verb, full, .. }
                if (full.ends_with("て") || full.ends_with("で"))
                && next_str != Some("は")
                && next_str != Some("も")
                && !matches!(
                    next_token.map(|t| t.base.as_str()),
                    Some("くれる" | "もらう" | "あげる" | "いる"
                       | "しまう" | "おく" | "みる" | "いく"
                       | "くる" | "ある")
                ) => {
                
                let mut relation = ClauseRelation::Continuation;
                let connective = if next_str == Some("から") {
                    let kara = tokens.get(i + 1).unwrap().clone();
                    i += 1; // Skip the "から" token in the main loop
                    relation = ClauseRelation::Sequence;
                    Some(kara)
                } else {
                    None
                };

                clauses.push(Clause { chunks, relation, connective, ending_particles: std::mem::take(&mut pending_ending_particles) });
                chunks = Vec::new();
                i += 1;
            }

            // Vるまで clause split
            ProcToken { pos: PartOfSpeech::Verb, .. } if next_str == Some("まで") => {
                let made_token = tokens.get(i + 1).unwrap().clone();
                clauses.push(Clause {
                    chunks,
                    relation: ClauseRelation::Until,
                    connective: Some(made_token),
                    ending_particles: std::mem::take(&mut pending_ending_particles),
                });
                chunks = Vec::new();
                i += 2;
            }

            // Relative clause: Verb immediately followed by Noun
            ProcToken { pos: PartOfSpeech::Verb, .. } if next_pos == Some(PartOfSpeech::Noun) => {
                let next = tokens.get(i + 1).unwrap();
                let is_ending_no = next.full == "の" && match tokens.get(i + 2) {
                    Some(n2) => n2.sub1 == PartOfSpeechSubcategory1::EndingParticle 
                             || n2.sub2 == PartOfSpeechSubcategory2::Quotation
                             || n2.full == "に", // Prevent relative clause split for のに
                    None => true,
                };
                
                if !is_ending_no {
                    let modifier_clause = Clause { chunks, relation: ClauseRelation::Modifier, connective: None, ending_particles: Vec::new() };
                    pending_modifiers.push(Modifier::Clause(Box::new(modifier_clause)));
                    chunks = Vec::new();
                }
                i += 1;
            }

            // Quotation と — Lindera reliably tags as sub2 == Quotation
            ProcToken { pos: PartOfSpeech::Particle, sub2: PartOfSpeechSubcategory2::Quotation, .. } => {
                let to_chunk = chunks.pop().unwrap(); // Remove the と
                let connective = Some(to_chunk.word);
                clauses.push(Clause { chunks, relation: ClauseRelation::Quotation, connective, ending_particles: std::mem::take(&mut pending_ending_particles) });
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
                    "ば" | "たら" | "と" => ClauseRelation::Conditional,
                    "ながら" => ClauseRelation::Simultaneous,
                    _ => ClauseRelation::Main,
                };
                let connective = chunks.pop().map(|c| c.word);
                clauses.push(Clause { chunks, relation, connective, ending_particles: std::mem::take(&mut pending_ending_particles) });
                chunks = Vec::new();
                i += 1;
            }

            ProcToken { pos: PartOfSpeech::Particle, sub1: PartOfSpeechSubcategory1::EndingParticle, .. } => {
                let ep_chunk = chunks.pop().unwrap();
                pending_ending_particles.push(ep_chunk.word);
                i += 1;
            }

            // F9: "の" tagged as Noun but acting as an ending particle
            ProcToken { pos: PartOfSpeech::Noun, .. } if current.full == "の" => {
                let is_ending_no = match tokens.get(i + 1) {
                    Some(n1) => n1.sub1 == PartOfSpeechSubcategory1::EndingParticle || n1.sub2 == PartOfSpeechSubcategory2::Quotation,
                    None => true,
                };
                if is_ending_no {
                    let ep_chunk = chunks.pop().unwrap();
                    pending_ending_particles.push(ep_chunk.word);
                }
                i += 1;
            }

            _ => { 
                i += 1; 
            }
        }
    }

    if !chunks.is_empty() || !pending_ending_particles.is_empty() {
        clauses.push(Clause {
            chunks: chunks,
            relation: ClauseRelation::Main,
            connective: None,
            ending_particles: std::mem::take(&mut pending_ending_particles),
        });
    }

    let mut sentence = Sentence { clauses };

    // Merge ending particles from a trailing empty clause into the previous clause
    if let Some(last) = sentence.clauses.last() {
        if last.chunks.is_empty() && !last.ending_particles.is_empty() {
            let last = sentence.clauses.pop().unwrap();
            if let Some(prev) = sentence.clauses.last_mut() {
                prev.ending_particles.extend(last.ending_particles);
            }
        }
    }

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
                    next_word.sub1 == PartOfSpeechSubcategory1::LinkingParticle ||
                    (next_word.sub1 == PartOfSpeechSubcategory1::AdverbialParticle && (next_word.full == "まで" || next_word.full == "も" || next_word.full == "より"))) 
                {
                    chunk.particle_role = match next_word.full.as_str() {
                        "が" => Some(ParticleRole::Subject),
                        "を" => Some(ParticleRole::Object),
                        "は" => Some(ParticleRole::Topic),
                        "に" => Some(ParticleRole::Ambiguous(vec![
                            ParticleRole::IndirectObject, 
                            ParticleRole::Destination,
                            ParticleRole::Temporal,
                            ParticleRole::Purpose,
                            // Agent: only valid with passive predicates. Re-add when ConjugationFeatures has is_passive.
                            // Adverbial: Lindera tags as AdverbializingParticle, never reaches this code path.
                        ])),
                        "へ" => Some(ParticleRole::Destination),
                        "で" => Some(ParticleRole::Ambiguous(vec![ParticleRole::LocationAction, ParticleRole::Means])),
                        "から" => Some(ParticleRole::Source),
                        "まで" => Some(ParticleRole::Limit),
                        "も" => Some(ParticleRole::Also),
                        "より" => Some(ParticleRole::ComparisonBase),
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
                    print!(" + {}", particle.full);
                }
                if let Some(role) = &chunk.particle_role {
                    match role {
                        ParticleRole::Ambiguous(candidates) => {
                            let names: Vec<String> = candidates.iter().map(|c| format!("{:?}", c)).collect();
                            print!(" [Ambiguous: {}]", names.join("/"));
                        }
                        _ => print!(" [{:?}]", role),
                    }
                }
                println!();
                
                for modifier in &chunk.modifiers {
                    match modifier {
                        Modifier::Adjective(adj) => println!("    │   └── mod: {}", adj.full),
                        Modifier::Limitation(lim_chunk) => {
                            print!("    │   └── lim: {}", lim_chunk.word.full);
                            if let Some(p) = &lim_chunk.particle {
                                print!(" {}", p.full);
                            }
                            println!();
                            for inner_mod in &lim_chunk.modifiers {
                                match inner_mod {
                                    Modifier::Adjective(adj) => println!("    │       └── mod: {}", adj.full),
                                    Modifier::Limitation(inner) => {
                                        print!("    │       └── lim: {}", inner.word.full);
                                        if let Some(p) = &inner.particle {
                                            print!(" {}", p.full);
                                        }
                                        println!();
                                    },
                                    Modifier::Clause(clause) => println!("    │       └── mod: [{}]", clause.text()),
                                }
                            }
                        },
                        Modifier::Clause(clause) => println!("    │   └── mod: [{}]", clause.text()),
                    }
                }
            }
            
            if let Some(conn) = &clause.connective {
                println!("    └── Connective: {}", conn.full);
            }
            if !clause.ending_particles.is_empty() {
                let eps: Vec<&str> = clause.ending_particles.iter().map(|p| p.full.as_str()).collect();
                println!("    └── Ending: {}", eps.join(""));
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
        let sentence = "もっと早く起きればよかったのにな";
        println!("\n=== {} ===", sentence);
        let tokens = analyze_sentence(sentence);
        let s = build_sentence(tokens).unwrap();
        s.print();
    }

    #[test]
    fn test_multiple() {
        let cases = vec![
            "静かな公園で有名な料理を食べた",
            "彼女の大切な友達の古い日記が見つかった",
            "早く行かないと電車に乗れないよ",
            "雨が降りそうだったのに傘を持ってこなかった",
            "もっと早く起きればよかったのにな",
            "彼は疲れているのに仕事を続けた",
            "お腹が空いたら何か食べてもいいよ",
            "急に雨が降ってきたので走って帰った",
            "彼女は静かに部屋を出て行った",
            "田中さんと鈴木さんと山田さんが来た",
            "「明日来られない」と彼女が悲しそうに言った",
            "これは私が昨日買った新しい本だ",
            "そんなに食べたら太るよ",
            "彼が来なかったので試合が始められなかった",
            "もし明日晴れたら海に行こうと思っている",
        ];

        for sentence in cases {
            println!("\n=== {} ===", sentence);
            let tokens = analyze_sentence(sentence);
            let s = build_sentence(tokens).unwrap();
            s.print();
        }
    }
}