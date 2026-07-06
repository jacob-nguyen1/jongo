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
        let mut text = String::new();
        for c in &self.chunks {
            text.push_str(&c.text());
        }
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
    pub secondary_particle: Option<ProcToken>,
    pub particle_role: Option<ParticleRole>,
    pub modifiers: Vec<Modifier>,
    pub is_head: bool,
}

impl Chunk {
    pub fn text(&self) -> String {
        let mut s = String::new();
        for modifier in &self.modifiers {
            match modifier {
                Modifier::Adjective(adj) => s.push_str(&adj.full),
                Modifier::Limitation(lim) => s.push_str(&lim.text()),
                Modifier::Clause(clause) => s.push_str(&clause.text()),
            }
        }
        
        if self.word.pos != PartOfSpeech::Symbol {
            s.push_str(&self.word.full);
            if let Some(p) = &self.particle {
                s.push_str(&p.full);
            }
            if let Some(p2) = &self.secondary_particle {
                s.push_str(&p2.full);
            }
        }
        s
    }
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
    Listing,          // や、と
    Temporal,         // に
    Purpose,          // に
    Agent,            // に
    Adverbial,        // に
    Scope,            // で
    Approximate,      // ぐらい
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
            word: tokens.get(i).unwrap().clone(),
            particle: None,
            secondary_particle: None,
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
                            Chunk { word: particle_token, particle: None, secondary_particle: None, particle_role: None, modifiers: Vec::new(), is_head: true }
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
                    Some("くれる" | "もらう" | "貰う" | "あげる" | "いる" | "居る"
                       | "しまう" | "おく" | "置く" | "みる" | "見る" | "いく" | "行く"
                       | "くる" | "来る" | "ある" | "有る")
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

            // verb ends in tara 
            ProcToken { pos: PartOfSpeech::Verb, full, .. } if full.ends_with("たら")=> {
                let relation = ClauseRelation::Conditional;
                let connective = chunks.pop().map(|c| c.word);
                clauses.push(Clause { chunks, relation, connective, ending_particles: std::mem::take(&mut pending_ending_particles) });
                chunks = Vec::new();
                i += 1;
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
                    "ば" | "と" => ClauseRelation::Conditional,
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
                    next_word.sub1 == PartOfSpeechSubcategory1::CoordinatingParticle ||
                    (next_word.sub1 == PartOfSpeechSubcategory1::AdverbialParticle && 
                     (next_word.full == "まで" || next_word.full == "も" || next_word.full == "より" || 
                      next_word.full == "ぐらい" || next_word.full == "くらい" || 
                      next_word.full == "ごろ" || next_word.full == "ころ")))
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
                        "で" => Some(ParticleRole::Ambiguous(vec![ParticleRole::LocationAction, ParticleRole::Means, ParticleRole::Scope])),
                        "から" => Some(ParticleRole::Source),
                        "まで" => Some(ParticleRole::Limit),
                        "も" => Some(ParticleRole::Also),
                        "より" => Some(ParticleRole::ComparisonBase),
                        "と" => Some(ParticleRole::Ambiguous(vec![ParticleRole::Accompaniment, ParticleRole::Listing])),
                        "や" => Some(ParticleRole::Listing),
                        "ぐらい" | "くらい" | "ごろ" | "ころ" => Some(ParticleRole::Approximate),
                        _ => None, // Unmapped particle
                    };
                    chunk.particle = Some(next_word.clone());
                    
                    // Consume the particle so it doesn't become a standalone chunk
                    iter.next();

                    // Check for double particle (は or も)
                    if let Some(next_next_chunk) = iter.peek() {
                        let nn_word = &next_next_chunk.word;
                        if nn_word.pos == PartOfSpeech::Particle && 
                           (nn_word.sub1 == PartOfSpeechSubcategory1::LinkingParticle || 
                           (nn_word.sub1 == PartOfSpeechSubcategory1::AdverbialParticle && nn_word.full == "も")) 
                        {
                            if nn_word.full == "は" || nn_word.full == "も" {
                                chunk.secondary_particle = Some(nn_word.clone());
                                iter.next();
                            }
                        }
                    }
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
            print_clause(clause, "");
        }
    }
}

fn print_clause(clause: &Clause, prefix: &str) {
    println!("{}└── Clause ({:?})", prefix, clause.relation);
    
    let child_prefix = format!("{}    ", prefix);
    let chunk_count = clause.chunks.len();
    
    for (i, chunk) in clause.chunks.iter().enumerate() {
        if chunk.word.pos == PartOfSpeech::Symbol {
            continue;
        }
        
        let is_last = i == chunk_count - 1 && clause.connective.is_none() && clause.ending_particles.is_empty();
        print_chunk(chunk, &child_prefix, is_last, false);
    }
    
    if let Some(conn) = &clause.connective {
        let is_last = clause.ending_particles.is_empty();
        let branch = if is_last { "└──" } else { "├──" };
        println!("{}{} Connective: {}", child_prefix, branch, conn.full);
    }
    if !clause.ending_particles.is_empty() {
        let eps: Vec<&str> = clause.ending_particles.iter().map(|p| p.full.as_str()).collect();
        println!("{}└── Ending: {}", child_prefix, eps.join(""));
    }
}

fn print_chunk(chunk: &Chunk, prefix: &str, is_last: bool, is_limitation: bool) {
    let branch = if is_last && chunk.modifiers.is_empty() { "└──" } else { "├──" };
    let node_type = if is_limitation { "lim" } else { "Chunk" };
    
    print!("{}{} {}: {}", prefix, branch, node_type, chunk.word.full);
    if let Some(particle) = &chunk.particle {
        let mut p_text = particle.full.clone();
        if let Some(p2) = &chunk.secondary_particle {
            p_text.push_str(&p2.full);
        }
        print!(" + {}", p_text);
    }
    if let Some(role) = &chunk.particle_role {
        match role {
            ParticleRole::Ambiguous(candidates) => {
                let names: Vec<String> = candidates.iter().map(|c| format!("{:?}", c)).collect();
                print!(" [Ambiguous: {}]", names.join("/"));
            }
            _ => print!(" [{:?}", role),
        }
        if let Some(p2) = &chunk.secondary_particle {
            if p2.full == "は" {
                print!(" + Topic");
            } else if p2.full == "も" {
                print!(" + Also");
            }
        }
        print!("]");
    }
    println!();
    
    let child_prefix = if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };
    
    let mod_count = chunk.modifiers.len();
    for (i, modifier) in chunk.modifiers.iter().enumerate() {
        let mod_is_last = i == mod_count - 1;
        let mod_branch = if mod_is_last { "└──" } else { "├──" };
        
        match modifier {
            Modifier::Adjective(adj) => println!("{}{} mod: {}", child_prefix, mod_branch, adj.full),
            Modifier::Limitation(lim_chunk) => {
                print_chunk(lim_chunk, &child_prefix, mod_is_last, true);
            },
            Modifier::Clause(clause) => {
                println!("{}{} mod: [Clause]", child_prefix, mod_branch);
                print_clause(clause, &format!("{}    ", child_prefix));
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
        let text = "それは1か月でした。";
        
        for sentence in text.split_inclusive('。') {
            let sentence = sentence.trim();
            if sentence.is_empty() { continue; }
            
            println!("\n=== {} ===", sentence);
            let tokens = analyze_sentence(sentence);
            if let Some(s) = build_sentence(tokens) {
                s.print();
            }
        }
    }

    #[test]
    fn test_multiple() {
        let cases = vec![
            "店にはペットボトルや缶が集まりました",
            "世界で有名な作家の村上春樹さんの新しい本が出ました。",
            "早く家に帰って読みたいです」と話していました。",
            "静かな公園を1時間ぐらい歩きながら、友達が好きな料理を食べたのに、彼女は急に家を出て行った",
            "3時ごろに店へ行く。3時ころに帰る。1時間くらい待つ。",
        ];

        for text in cases {
            for sentence in text.split_inclusive('。') {
                let sentence = sentence.trim();
                if sentence.is_empty() { continue; }
                
                println!("\n=== {} ===", sentence);
                let tokens = analyze_sentence(sentence);
                let s = build_sentence(tokens).unwrap();
                s.print();
            }
        }
    }
}