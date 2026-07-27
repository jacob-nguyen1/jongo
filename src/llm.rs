use crate::sentence::{Sentence, Chunk, Modifier};
use crate::labels::ParticleRole;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct LlmResponse {
    pub disambiguations: Vec<LlmDisambiguation>,
}

#[derive(Deserialize, Debug)]
pub struct LlmDisambiguation {
    pub chunk_id: usize,
    #[serde(rename = "type")]
    pub disambiguation_type: String, // "particle_role" or "vocabulary"
    pub result: serde_json::Value, // String for particle_role, integer for vocabulary
}

pub fn generate_prompt(ast: &Sentence, sentence_str: &str, context: &str) -> String {
    let mut ambiguous_particles: Vec<(usize, String, Vec<String>)> = Vec::new();
    let mut vocabulary: Vec<(usize, String, Vec<String>)> = Vec::new();
    let mut chunk_id = 0usize;

    fn process_chunk(
        chunk: &Chunk,
        chunk_id: &mut usize,
        ambiguous_particles: &mut Vec<(usize, String, Vec<String>)>,
        vocabulary: &mut Vec<(usize, String, Vec<String>)>,
    ) {
        // Process nested modifiers FIRST (same order as render_chunk_group)
        for modif in &chunk.modifiers {
            match modif {
                Modifier::NounChunk(c) | Modifier::AdjectiveChunk(c) | Modifier::AdverbChunk(c) | Modifier::Limitation(c) => {
                    process_chunk(c, chunk_id, ambiguous_particles, vocabulary);
                }
                Modifier::Clause(c) => {
                    process_chunk(&c.predicate, chunk_id, ambiguous_particles, vocabulary);
                }
                Modifier::Quotation(s) => {
                    process_sentence(s, chunk_id, ambiguous_particles, vocabulary);
                }
            }
        }

        // Then assign ID for this chunk's head (matches render_row call order)
        let id = *chunk_id;
        *chunk_id += 1;

        // Collect ambiguous particle roles
        if let Some(ParticleRole::Ambiguous(candidates)) = &chunk.particle_role {
            if let Some(p) = &chunk.particle {
                let candidate_strs: Vec<String> = candidates.iter().map(|c| c.badge().to_string()).collect();
                ambiguous_particles.push((id, p.full.clone(), candidate_strs));
            }
        }

        // Collect vocabulary with multiple definitions (skip particles)
        let is_proper_noun = chunk.word.sub1 == crate::labels::PartOfSpeechSubcategory1::ProperNoun;
        let is_particle = chunk.word.pos == crate::labels::PartOfSpeech::Particle;
        let glosses: Vec<String> = crate::jmdict::lookup(&chunk.word.base, chunk.word.pos.clone(), is_proper_noun)
            .into_iter()
            .flat_map(|r| r.glosses)
            .collect();

        if glosses.len() > 1 && !is_particle {
            vocabulary.push((id, chunk.word.full.clone(), glosses));
        }
    }

    fn process_sentence(
        sentence: &Sentence,
        chunk_id: &mut usize,
        ambiguous_particles: &mut Vec<(usize, String, Vec<String>)>,
        vocabulary: &mut Vec<(usize, String, Vec<String>)>,
    ) {
        for clause in &sentence.clauses {
            process_chunk(&clause.predicate, chunk_id, ambiguous_particles, vocabulary);
        }
    }

    process_sentence(ast, &mut chunk_id, &mut ambiguous_particles, &mut vocabulary);

    let mut prompt = String::new();
    if !context.is_empty() && context != sentence_str {
        prompt.push_str(&format!("Surrounding Context Paragraph:\n\"{}\"\n\n", context));
    }
    prompt.push_str(&format!("Target Sentence to Analyze:\n'{}'\n\n", sentence_str));
    prompt.push_str("I need you to disambiguate grammatical particle roles AND vocabulary definitions based on the sentence and context.\n\n");
    prompt.push_str("Output a JSON object containing an array named 'disambiguations'. Do NOT wrap the output in markdown codeblocks like ```json, just output the raw JSON.\n");
    prompt.push_str("Each item in the array should have:\n");
    prompt.push_str("1. 'chunk_id': The integer chunk ID provided below.\n");
    prompt.push_str("2. 'type': Either 'particle_role' or 'vocabulary'.\n");
    prompt.push_str("3. 'result': The chosen role (exact string from the candidates) OR the chosen definition index (integer).\n\n");

    if !ambiguous_particles.is_empty() {
        prompt.push_str("For these ambiguous particles, select the correct role from the candidates:\n");
        for (id, particle, candidates) in &ambiguous_particles {
            prompt.push_str(&format!("- chunk_id {}: particle '{}', candidates: {:?}\n", id, particle, candidates));
        }
        prompt.push_str("\n");
    }

    if !vocabulary.is_empty() {
        prompt.push_str("For these words, select the integer index of the correct dictionary sense:\n");
        for (id, word, defs) in &vocabulary {
            prompt.push_str(&format!("- chunk_id {}: '{}':\n", id, word));
            for (i, def) in defs.iter().enumerate() {
                prompt.push_str(&format!("    {}: {}\n", i, def));
            }
        }
    }

    prompt
}
