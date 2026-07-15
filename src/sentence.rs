use crate::labels::{PartOfSpeech, PartOfSpeechSubcategory1, PartOfSpeechSubcategory2, ClauseRelation, ParticleRole};
use crate::grammar::{analyze_sentence, ProcToken};

#[derive(Debug)]
pub struct Sentence {
    pub clauses: Vec<Clause>,
}

#[derive(Debug)]
pub struct Clause {
    pub predicate: Chunk,
    pub relation: ClauseRelation,
    pub connective: Option<ProcToken>,
    pub ending_particles: Vec<ProcToken>,
}

impl Clause {
    pub fn text(&self) -> String {
        let mut text = String::new();
        text.push_str(&self.predicate.text());
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
                Modifier::NounChunk(arg) => s.push_str(&arg.text()),
                Modifier::AdjectiveChunk(adj) => s.push_str(&adj.text()),
                Modifier::Limitation(lim) => s.push_str(&lim.text()),
                Modifier::Clause(clause) => s.push_str(&clause.text()),
                Modifier::AdverbChunk(adv) => s.push_str(&adv.text()),
                Modifier::Quotation(sentence) => {
                    s.push_str("「");
                    for clause in &sentence.clauses {
                        s.push_str(&clause.text());
                    }
                    s.push_str("」と");
                }
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
    NounChunk(Box<Chunk>),
    AdjectiveChunk(Box<Chunk>),
    Limitation(Box<Chunk>),
    Clause(Box<Clause>),
    AdverbChunk(Box<Chunk>),
    Quotation(Box<Sentence>),
}

fn extract_adverbs(chunks: &mut Vec<Chunk>) -> Vec<Modifier> {
    assign_particle_roles(chunks);
    let mut adverbs = Vec::new();
    while let Some(c) = chunks.last() {
        if c.word.pos == PartOfSpeech::Adverb || c.particle_role == Some(ParticleRole::Adverbial) {
            adverbs.insert(0, Modifier::AdverbChunk(Box::new(chunks.pop().unwrap())));
        } else {
            break;
        }
    }
    adverbs
}



pub fn build_sentence(tokens: Vec<ProcToken>) -> Option<Sentence> {
    let tokens: Vec<ProcToken> = tokens
        .into_iter()
        .filter(|t| t.pos != PartOfSpeech::Symbol || 
                    t.sub1 == PartOfSpeechSubcategory1::Comma ||
                    t.sub1 == PartOfSpeechSubcategory1::OpenParenthesis ||
                    t.sub1 == PartOfSpeechSubcategory1::ClosedParenthesis)
        .collect();

    let mut clauses: Vec<Clause> = Vec::new();
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut pending_modifiers: Vec<Modifier> = Vec::new();
    let mut pending_ending_particles: Vec<ProcToken> = Vec::new();
    let mut comma_barrier: Option<usize> = None; // chunks.len() at time of last comma
    
    let mut i = 0;
    while i < tokens.len() {
        let current = &tokens[i];

        // Comma barrier: record position in chunks vec, skip the comma token
        if current.pos == PartOfSpeech::Symbol && current.sub1 == PartOfSpeechSubcategory1::Comma {
            comma_barrier = Some(chunks.len());
            i += 1;
            continue;
        }

        // Bracketed Quotation start: capture until CloseParenthesis
        if current.pos == PartOfSpeech::Symbol && current.sub1 == PartOfSpeechSubcategory1::OpenParenthesis {
            let mut inner_tokens = Vec::new();
            let mut j = i + 1;
            let mut found_close = false;
            while j < tokens.len() {
                if tokens[j].pos == PartOfSpeech::Symbol && tokens[j].sub1 == PartOfSpeechSubcategory1::ClosedParenthesis {
                    found_close = true;
                    break;
                }
                inner_tokens.push(tokens[j].clone());
                j += 1;
            }
            if found_close {
                if let Some(inner_sentence) = build_sentence(inner_tokens) {
                    chunks.push(Chunk {
                        word: current.clone(), // Use the opening parenthesis as the dummy head
                        particle: None,
                        secondary_particle: None,
                        particle_role: None,
                        modifiers: vec![Modifier::Quotation(Box::new(inner_sentence))],
                        is_head: false,
                    });
                }
                i = j + 1; // Skip past the closing parenthesis
                
                // If there's a following `と`, skip it too because it's the quotation particle
                if i < tokens.len() && tokens[i].full == "と" && tokens[i].sub2 == PartOfSpeechSubcategory2::Quotation {
                    i += 1;
                }
                continue;
            }
        }
        
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
            ProcToken { pos: PartOfSpeech::Adjective, conjugation, full, .. }
                if conjugation.as_ref().map_or(false, |c| c.teform)
                && (full.ends_with("て") || full.ends_with("で")) => {
                clauses.push(package_clause(chunks, ClauseRelation::Continuation, None, std::mem::take(&mut pending_ending_particles)));
                chunks = Vec::new();
                i += 1;
            }

            // い-adjective modifier
            ProcToken { pos: PartOfSpeech::Adjective, .. } if next_pos == Some(PartOfSpeech::Noun) => {
                assign_particle_roles(&mut chunks);
                let mut adj_chunk = chunks.pop().unwrap();
                let adverbs = extract_adverbs(&mut chunks);
                
                let extracted = std::mem::take(&mut adj_chunk.modifiers);
                let (nested_adverbs, others): (Vec<_>, Vec<_>) = extracted.into_iter().partition(|m| matches!(m, Modifier::AdverbChunk(_)));
                
                adj_chunk.modifiers = nested_adverbs;
                adj_chunk.modifiers.extend(adverbs);
                
                pending_modifiers.extend(others);
                pending_modifiers.push(Modifier::AdjectiveChunk(Box::new(adj_chunk)));
                i += 1;
            }

            // な-adjective modifier (e.g. 静か + な + 公園)
            ProcToken { pos: PartOfSpeech::Noun, sub1: PartOfSpeechSubcategory1::AdjectiveVerbStem, .. } 
                if next_token.map_or(false, |t| t.base == "だ" && t.full == "な") && tokens.get(i + 2).map(|t| t.pos) == Some(PartOfSpeech::Noun) => {
                // If it's the ending 'の', we shouldn't trigger the modifier rule.
                let is_ending_no = tokens.get(i + 2).unwrap().full == "の" && match tokens.get(i + 3) {
                    Some(n3) => n3.sub1 == PartOfSpeechSubcategory1::EndingParticle 
                             || n3.sub2 == PartOfSpeechSubcategory2::Quotation
                             || n3.full == "に",
                    None => true,
                };
                
                if !is_ending_no {
                    assign_particle_roles(&mut chunks);
                    let mut adj_chunk = chunks.pop().unwrap();
                    adj_chunk.particle = Some(next_token.unwrap().clone());
                    let adverbs = extract_adverbs(&mut chunks);
                    
                    let extracted = std::mem::take(&mut adj_chunk.modifiers);
                    let (nested_adverbs, others): (Vec<_>, Vec<_>) = extracted.into_iter().partition(|m| matches!(m, Modifier::AdverbChunk(_)));
                    
                    adj_chunk.modifiers = nested_adverbs;
                    adj_chunk.modifiers.extend(adverbs);
                    
                    pending_modifiers.extend(others);
                    pending_modifiers.push(Modifier::AdjectiveChunk(Box::new(adj_chunk)));
                }
                i += 2; // Skips both the AdjectiveVerbStem AND the absorbed 'な'
            }

            // のに concessive clause split
            ProcToken { pos: PartOfSpeech::Noun, sub1: PartOfSpeechSubcategory1::Bound, full, .. }
                if full == "の" && next_str == Some("に") => {
                let no_chunk = chunks.pop().unwrap();
                let mut connective_token = no_chunk.word;
                connective_token.full = format!("{}に", connective_token.full); // combine into "のに"
                clauses.push(package_clause(chunks, ClauseRelation::Concessive, Some(connective_token), std::mem::take(&mut pending_ending_particles)));
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

                    
                    pending_modifiers.push(Modifier::Limitation(Box::new(lim_chunk)));
                }
                i += 1;
            }

            // === TEMPORAL 時 CLAUSE SEPARATION ===
            // When 時 has preceding modifiers (verb clause, の-linked noun, adjective), it marks
            // a temporal subordinate clause boundary.
            // Triggers when followed by: a particle (に, は, も, から, まで) OR no particle at all
            // (casual speech: 子供の時よく遊んだ).
            // Does NOT trigger when followed by の (時 is linking to another noun, e.g. 行った時のこと).
            // Standalone 時には ("sometimes") is pre-merged by Lindera into a single Adverb token,
            // so it never reaches this rule. その時 has no modifiers, so it also doesn't trigger.
            ProcToken { pos: PartOfSpeech::Noun, sub1: PartOfSpeechSubcategory1::Bound, sub2: PartOfSpeechSubcategory2::PossibleAdverb, .. }
                if current.base == "時" 
                && !chunks.last().map_or(true, |c| c.modifiers.is_empty())
                && next_str != Some("の") => {
                
                // Pop the 時 chunk — it becomes the connective
                let toki_chunk = chunks.pop().unwrap();
                
                // Extract modifiers from the 時 chunk back into the clause.
                // e.g. 子供の時 → 子供 chunk stays in temporal clause
                // e.g. 行った時 → 行った clause's chunks get extracted into temporal clause
                for modifier in toki_chunk.modifiers {
                    match modifier {
                        Modifier::Clause(inner_clause) => {
                            chunks.push(inner_clause.predicate);
                        }
                        Modifier::Limitation(lim_chunk) => {
                            chunks.push(*lim_chunk);
                        }
                        Modifier::AdjectiveChunk(adj_chunk) => {
                            chunks.push(*adj_chunk);
                        }
                        _ => {}
                    }
                }
                
                // Build connective: 時 + following boundary particles only (に, は, も, から, まで)
                let mut connective_surface = toki_chunk.word.full.clone();
                let mut consumed = 0;
                let mut j = i + 1;
                while j < tokens.len() && matches!(tokens[j].full.as_str(), "に" | "は" | "も" | "から" | "まで") {
                    connective_surface.push_str(&tokens[j].full);
                    consumed += 1;
                    j += 1;
                }
                
                let mut connective_token = toki_chunk.word;
                connective_token.full = connective_surface;
                
                clauses.push(package_clause(chunks, ClauseRelation::Temporal, Some(connective_token), std::mem::take(&mut pending_ending_particles)));
                chunks = Vec::new();
                i += 1 + consumed;
            }

            // === CLAUSE SEPARATION ===

            // te-form verb marks continuation
            // りんごを食べて水を飲んだ
            ProcToken { pos: PartOfSpeech::Verb, conjugation, full, .. }
                if conjugation.as_ref().map_or(false, |c| c.teform)
                && (full.ends_with("て") || full.ends_with("で"))
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

                clauses.push(package_clause(chunks, relation, connective, std::mem::take(&mut pending_ending_particles)));
                chunks = Vec::new();
                i += 1;
            }

            // Vるまで clause split
            ProcToken { pos: PartOfSpeech::Verb, .. } if next_str == Some("まで") => {
                let made_token = tokens.get(i + 1).unwrap().clone();
                clauses.push(package_clause(chunks, ClauseRelation::Until, Some(made_token), std::mem::take(&mut pending_ending_particles)));
                chunks = Vec::new();
                i += 2;
            }

            // verb ends in tara 
            ProcToken { pos: PartOfSpeech::Verb, full, .. } if full.ends_with("たら")=> {
                let relation = ClauseRelation::Conditional;
                let connective = chunks.pop().map(|c| c.word);
                clauses.push(package_clause(chunks, relation, connective, std::mem::take(&mut pending_ending_particles)));
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
                    // Comma barrier: only include post-comma chunks in the modifier clause.
                    // Pre-comma chunks stay in the main clause (e.g. 昨日、買った本 → 昨日 stays out).
                    let modifier_chunks = if let Some(barrier) = comma_barrier.take() {
                        chunks.drain(barrier..).collect()
                    } else {
                        std::mem::take(&mut chunks)
                    };
                    let modifier_clause = package_clause(modifier_chunks, ClauseRelation::Modifier, None, Vec::new());
                    pending_modifiers.push(Modifier::Clause(Box::new(modifier_clause)));
                }
                i += 1;
            }

            // Quotation と — Lindera reliably tags as sub2 == Quotation
            ProcToken { pos: PartOfSpeech::Particle, sub2: PartOfSpeechSubcategory2::Quotation, .. } => {
                let to_chunk = chunks.pop().unwrap(); // Remove the と
                let connective = Some(to_chunk.word);
                clauses.push(package_clause(chunks, ClauseRelation::Quotation, connective, std::mem::take(&mut pending_ending_particles)));
                chunks = Vec::new();
                i += 1;
            }

            // Standalone conjunctive particles
            // 雨が降っているので行きません。
            ProcToken { pos: PartOfSpeech::Particle, sub1: PartOfSpeechSubcategory1::ConjuctiveParticle, full, .. } => {
                let is_niyoruto = full == "と" && chunks.len() >= 2 && chunks[chunks.len() - 2].word.base == "よる";

                let relation = if is_niyoruto {
                    ClauseRelation::Evidential
                } else {
                    match full.as_str() {
                        "から" | "ので" => ClauseRelation::Reason,
                        "けど" | "が" => ClauseRelation::Contrast,
                        "のに" => ClauseRelation::Concessive,
                        "ば" => ClauseRelation::Conditional,
                        "と" => ClauseRelation::Ambiguous(vec![ClauseRelation::Conditional, ClauseRelation::Quotation]),
                        "ながら" => ClauseRelation::Simultaneous,
                        _ => ClauseRelation::Main,
                    }
                };
                let connective = chunks.pop().map(|c| c.word);
                clauses.push(package_clause(chunks, relation, connective, std::mem::take(&mut pending_ending_particles)));
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
        clauses.push(package_clause(chunks, ClauseRelation::Main, None, std::mem::take(&mut pending_ending_particles)));
    }

    let mut sentence = Sentence { clauses };

    // Merge ending particles from a trailing empty clause into the previous clause
    if let Some(last) = sentence.clauses.last() {
        if last.predicate.word.full.is_empty() && !last.ending_particles.is_empty() {
            let last = sentence.clauses.pop().unwrap();
            if let Some(prev) = sentence.clauses.last_mut() {
                prev.ending_particles.extend(last.ending_particles);
            }
        }
    }

    // Pass 2 is no longer needed since package_clause handles particle roles internally!

    Some(sentence)
}

fn package_clause(mut chunks: Vec<Chunk>, relation: ClauseRelation, connective: Option<ProcToken>, ending_particles: Vec<ProcToken>) -> Clause {
    assign_particle_roles(&mut chunks);
    
    if chunks.is_empty() {
        // Fallback for trailing ending particles on an empty clause
        return Clause {
            predicate: Chunk {
                word: ProcToken { full: "".to_string(), base: "".to_string(), pos: PartOfSpeech::Symbol, sub1: PartOfSpeechSubcategory1::X, sub2: PartOfSpeechSubcategory2::X, conjugation: None, staircase: None },
                particle: None, secondary_particle: None, particle_role: None, modifiers: Vec::new(), is_head: true
            },
            relation, connective, ending_particles
        };
    }
    
    let mut predicate = chunks.pop().unwrap();
    
    let mut dependents = Vec::new();
    for mut c in chunks {
        if c.modifiers.len() == 1 && matches!(c.modifiers[0], Modifier::Quotation(_)) {
            dependents.push(c.modifiers.pop().unwrap());
        } 
        else if c.word.pos == PartOfSpeech::Adverb || c.particle_role == Some(ParticleRole::Adverbial) {
            dependents.push(Modifier::AdverbChunk(Box::new(c)));
        } 
        else {
            dependents.push(Modifier::NounChunk(Box::new(c)));
        }
    }
    
    dependents.extend(predicate.modifiers);
    predicate.modifiers = dependents;
    
    Clause {
        predicate,
        relation,
        connective,
        ending_particles,
    }
}

fn assign_particle_roles(chunks: &mut Vec<Chunk>) {
    let mut new_chunks = Vec::new();
    let old_chunks = std::mem::take(chunks);
    let mut iter = old_chunks.into_iter().peekable();
    
    while let Some(mut chunk) = iter.next() {
        if chunk.word.pos == PartOfSpeech::Noun || chunk.word.pos == PartOfSpeech::Adverb {
            if let Some(next_chunk) = iter.peek() {
                let next_word = &next_chunk.word;

                // Adverbial に (e.g. 静かに, 急に): Lindera tags this に as AdverbializingParticle,
                // distinct from regular MarkingParticle. Assign Adverbial directly, no ambiguity.
                if next_word.pos == PartOfSpeech::Particle &&
                   next_word.sub1 == PartOfSpeechSubcategory1::AdverbializingParticle {
                    chunk.particle_role = Some(ParticleRole::Adverbial);
                    chunk.particle = Some(next_word.clone());
                    iter.next();
                }
                // Standard particle absorption
                else if next_word.pos == PartOfSpeech::Particle &&
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
                        "に" if chunk.word.sub1 == PartOfSpeechSubcategory1::Adverbial => Some(ParticleRole::Adverbial),
                        "に" => Some(ParticleRole::Ambiguous(vec![
                            ParticleRole::IndirectObject, 
                            ParticleRole::Destination,
                            ParticleRole::Temporal,
                            ParticleRole::Purpose,
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

        new_chunks.push(chunk);
    }
    *chunks = new_chunks;
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
    print_chunk(&clause.predicate, &child_prefix, "└──", false, false);
    
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

pub fn print_chunk(chunk: &Chunk, prefix: &str, branch: &str, is_limitation: bool, is_adverb: bool) {
    // Modifiers must have a vertical line connecting them down to the Root chunk
    let mod_child_prefix = format!("{}│   ", prefix);

    // Print modifiers BEFORE the head chunk to match sequential UI rendering
    for (i, modifier) in chunk.modifiers.iter().enumerate() {
        let mod_is_first = i == 0; 
        let mod_branch = if mod_is_first { "┌──" } else { "├──" };
        
        match modifier {
            Modifier::NounChunk(arg_chunk) => {
                print_chunk(arg_chunk, &mod_child_prefix, mod_branch, false, false);
            }
            Modifier::AdjectiveChunk(adj_chunk) => {
                print_chunk(adj_chunk, &mod_child_prefix, mod_branch, false, false);
            }
            Modifier::Clause(mod_clause) => {
                println!("{}{} mod: [Clause]", mod_child_prefix, mod_branch);
                print_clause(mod_clause, &format!("{}    ", mod_child_prefix));
            }
            Modifier::Limitation(lim_chunk) => {
                print_chunk(lim_chunk, &mod_child_prefix, mod_branch, true, false);
            }
            Modifier::AdverbChunk(adv_chunk) => {
                print_chunk(adv_chunk, &mod_child_prefix, mod_branch, false, true);
            }
            Modifier::Quotation(quote_sentence) => {
                println!("{}{} mod: [Quotation]", mod_child_prefix, mod_branch);
                for c in &quote_sentence.clauses {
                    print_clause(c, &format!("{}    ", mod_child_prefix));
                }
            }
        }
    }

    let node_type = if is_limitation { "lim" } else if is_adverb { "adv" } else { "n" };
    
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::analyze_sentence;

    #[test]
    fn test() {
        // Fix 4: Comma barrier. 
        // 1st case: 昨日 is pulled into the relative clause.
        // 2nd case: 昨日 stays in the main clause.
        // Fix 5: によると evidential expression
        // Fix 6: Bracketed quotation
        let text = "昨日買った本を読む。昨日、買った本を読む。子供の時、よく遊んだ。天気予報によると明日は雨だ。彼は「行きたくない」と言った。";
        
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