use crate::sentence::{Sentence, Clause, Chunk, Modifier};
use crate::labels::ParticleRole;

pub fn generate_prompt(ast: &Sentence, sentence_str: &str) -> String {
    let mut ambiguous_particles = Vec::new();
    let mut vocabulary = Vec::new();

    fn process_chunk(chunk: &Chunk, ambiguous_particles: &mut Vec<String>, vocabulary: &mut Vec<(String, Vec<String>)>) {
        // Collect ambiguous particle roles
        if let Some(ParticleRole::Ambiguous(candidates)) = &chunk.particle_role {
            if let Some(p) = &chunk.particle {
                let candidate_strs: Vec<String> = candidates.iter().map(|c| format!("{:?}", c)).collect();
                ambiguous_particles.push(format!("'{}': {:?}", p.full, candidate_strs));
            }
        }
        
        // Collect vocabulary with multiple definitions
        let is_proper_noun = chunk.word.sub1 == crate::labels::PartOfSpeechSubcategory1::ProperNoun;
        let glosses: Vec<String> = crate::jmdict::lookup(&chunk.word.base, chunk.word.pos.clone(), is_proper_noun)
            .into_iter()
            .flat_map(|r| r.glosses)
            .collect();
            
        if glosses.len() > 1 {
            vocabulary.push((chunk.word.full.clone(), glosses));
        }
        
        // Process nested modifiers
        for modif in &chunk.modifiers {
            match modif {
                Modifier::NounChunk(c) | Modifier::AdjectiveChunk(c) | Modifier::AdverbChunk(c) | Modifier::Limitation(c) => process_chunk(c, ambiguous_particles, vocabulary),
                Modifier::Clause(c) => process_chunk(&c.predicate, ambiguous_particles, vocabulary),
                Modifier::Quotation(s) => process_sentence(s, ambiguous_particles, vocabulary),
            }
        }
    }
    
    fn process_sentence(sentence: &Sentence, ambiguous_particles: &mut Vec<String>, vocabulary: &mut Vec<(String, Vec<String>)>) {
        for clause in &sentence.clauses {
            process_chunk(&clause.predicate, ambiguous_particles, vocabulary);
        }
    }
    
    process_sentence(ast, &mut ambiguous_particles, &mut vocabulary);
    
    let mut prompt = format!("Analyze the Japanese sentence: '{}'.\n", sentence_str);
    prompt.push_str("I need you to disambiguate grammatical particle roles AND vocabulary definitions based on the sentence context.\n\n");
    prompt.push_str("Output a JSON array named 'disambiguations'. Each item in the array should have:\n");
    prompt.push_str("1. 'token': The exact word or particle.\n");
    prompt.push_str("2. 'type': Either 'particle_role' or 'vocabulary'.\n");
    prompt.push_str("3. 'result': The chosen role (string) OR the chosen definition index (integer).\n\n");
    
    if !ambiguous_particles.is_empty() {
        prompt.push_str("For these ambiguous particles, select the correct role:\n");
        for p in ambiguous_particles {
            prompt.push_str(&format!("- {}\n", p));
        }
        prompt.push_str("\n");
    }
    
    if !vocabulary.is_empty() {
        prompt.push_str("For these words, select the integer index of the correct dictionary sense:\n");
        for (word, defs) in vocabulary {
            prompt.push_str(&format!("- '{}':\n", word));
            for (i, def) in defs.iter().enumerate() {
                prompt.push_str(&format!("    {}: {}\n", i, def));
            }
        }
    }
    
    prompt
}
