# Jongo — Master TODO

## Extension UI (lib.rs)

### Current State
- WASM browser extension injected via content script
- Shift+hover detects sentence boundaries, shows "jong" prompt button
- Clicking "jong" opens a flat word-by-word gloss (token, kana, definitions)
- No clause structure, no particle roles, no hover interaction in the analysis window

### Primary View Mode (Build First)
The analysis window should present the parsed sentence as a **clause-segmented, interactive breakdown**.

**Layout:**
- Analysis window opens near the original text (current popup positioning is fine)
- Top of window: the full sentence, re-rendered with **clause boundaries** visually marked (color bands, subtle dividers, or background tints per clause)
- Below: clause-by-clause breakdown, each clause as a collapsible section showing its chunks

**Per-Clause Display:**
- Clause relation label visible (Reason, Contrast, Continuation, etc.)
- Connective particle shown at the clause boundary (けど, から, て, etc.)
- Each chunk displayed as a unit: word + particle + role badge (e.g. `友達 が [Subject]`)
- Modifiers (adjective, limitation, relative clause) indented or nested under their head chunk
- Definitions: each word shows reading (kana) and English gloss inline or on hover

**Hover Interaction:**
- Hovering a word in the analysis window highlights the corresponding text in the original page
- Hovering a word in the analysis window also highlights its structural relationships:
  - The chunk it belongs to
  - The clause it belongs to
  - Its modifier attachments (if any)
- Shift+hover on original page text could highlight the corresponding chunk in the analysis window (bidirectional)

**Word Detail (on hover or click):**
- Reading (kana)
- Dictionary form (base)
- Part of speech
- English definition(s) from JMdict
- Particle role (if chunk has a particle)
- Conjugation info (negative, past, te-form) when ConjugationFeatures are populated (G3)

### Future View Modes (Deferred)
- **Minimal mode:** Just clause-colored original text with hover glosses, no analysis panel
- **Tree mode:** Full AST tree visualization (modifier nesting, clause hierarchy)
- **Comparison mode:** Side-by-side original vs structural breakdown

### Implementation Notes
- `analyze()` in lib.rs currently calls `grammar::analyze_sentence()` directly. It needs to call the sentence parser (`sentence::build_sentence()`) instead, and render the `Sentence` AST into HTML.
- The `Sentence` struct needs a `to_html()` or similar method, or lib.rs builds HTML by walking the AST.
- Clause colors should be deterministic per relation type (e.g. Reason=blue, Contrast=orange, Main=neutral).
- Hover state management: track which chunk/clause is hovered, apply CSS classes to both analysis window elements and original page text spans.
- Original text needs to be wrapped in spans (per-token or per-chunk) during sentence detection so hover highlighting can target them.

---


## sentence.rs

### Actionable Fixes
- ~~**[F2] Eager modifier attachment:** ✅ Fixed. Eager modifiers (Adjectives, Clauses) attached to non-head nouns (captured by `の`) are extracted and bubbled up to the true head noun.~~
- ~~**[F3] `の` pops wrong chunk:** ✅ Fixed. `の` now pops past particles to find the noun. Particle stored on the noun chunk.~~
- ~~**[F6] Sentence-final ending particles:** ✅ Fixed. `ending_particles: Vec<ProcToken>` on `Clause`. EndingParticle tokens collected instead of becoming standalone chunks.~~
- ~~**[F7] Limitation print gap:** ✅ Fixed. `print()` now recurses into limitation chunk modifiers.~~
- **[F8] Double marking particles (`には`, `では`):** `assign_particle_roles` only absorbs one particle per noun. `には` = に role + は topicalization. `では` = で role + は topicalization. **Fix:** After absorbing the first particle, peek for は/も as a second particle. Add `is_topicalized: bool` to `Chunk` (or similar).
- **[F9] Stacked ending particles (`のよ`, `のね`):** Lindera tags の as `Noun, Bound` (nominalizer) when followed by another ending particle. Verb+Noun rule fires and treats の as a relative clause head. **Fix:** After Verb+Noun relative clause detection, check if the "noun" is Bound の and the token after it is EndingParticle or end of sentence — if so, treat の as an ending particle, not a nominalizer.

### Deferred & Ambiguities (LLM / Future Rules)
- **Compound particles:** Hardcode particles like `でも` as individual `ParticleRole` variants. Merge in `grammar.rs`.
- **Contrast `が` Edge Cases:** `が` (subject vs contrast) usually distinguished by Lindera, but edge cases possible.
- **Long `て` chains:** May lose semantic boundaries if too long.
- **Stacked relative clauses:** Only single modifier supported. Need to handle multiple modifiers on one noun.
- **Nested/Interjection quotation:** Reported speech within reported speech not handled. Interjection quotes (`「うん」と`) have no predicate and may misparse.
- **`も` after `て`-form:** (e.g. `てもいい`) not a particle role but might be incorrectly assigned.
- **Ambiguous roles requiring LLM/context:**
  - `LocationAction` vs `Means` (`で`)
  - `Source` (`から`) vs `TemporalStart` (needs noun type check)
  - `Limit` (`まで`) vs `TemporalLimit` (needs noun type check)
  - `Accompaniment` (`と`) vs `Listing` (`と`) (needs animacy detection)
  - `Destination` (`に`) vs `IndirectObject` (`に`) — indistinguishable from Lindera
- **Agent `に` detection:** Re-add Agent to に candidates when clause predicate is passive. Blocked on ConjugationFeatures gaining `is_passive`. Passive morphology (れる/られる) is merged into verb surface by grammar.rs, making string-matching unreliable (e.g. 忘れた false positive).
- **Formal `より` ("from"):** Identical to comparison `より`, mislabeled as comparison. Low priority.

## grammar.rs

### Actionable Fixes
- **[G2] Arabic numeral counters:** `2つ目と3つ目` parsed incorrectly (`目と` as suru verb). Kanji numerals (`二つ目`) work. **Fix:** Pre-process string to convert arabic numerals to kanji in counter contexts before Lindera call.

### Teammate Responsibility
- **[G3] ConjugationFeatures:** Populate `is_negative`, `is_past`, `is_te_form` as tokens merge. `sentence.rs` will later check `token.conjugation.is_te_form`.

## Particle Ambiguity Findings (に, で, より, から)
- **に (Ni):** Adverbial に is `AdverbializingParticle` (separate from MarkingParticle, never enters ambiguity). Agent requires passive predicate detection (blocked on ConjugationFeatures). Remaining 4-way ambiguity (IndirectObject/Destination/Temporal/Purpose) deferred to LLM.
- **で (De):** Conjunctive で (`親切で`) is `AuxiliaryVerb` (だ), not a particle. No collision with LocationAction/Means.
- **より (Yori):** Adverbial (`Adverb, General`) and Comparison (`Particle, MarkingParticle, General`) are distinct. Formal ("from") matches comparison.
- **から (Kara):** Temporal vs. Spatial is unresolvable (`Particle, MarkingParticle, General`). Defer to LLM.

## Lindera Subcategory Findings
- **と:** `MarkingParticle, Quotation` (Quotation), `MarkingParticle, General` (Accompaniment), `CoordinatingParticle` (Listing), `ConjunctiveParticle` (Conditional).
- **に:** `AdverbializingParticle` (Adverbial, e.g. 静かに), `MarkingParticle, General` (all other roles).
- **の, ね, よ, わ, な:** `EndingParticle` (Sentence-final).
- **の:** `Noun, Bound` (Nominalizing), `NormalizingParticle` (Structural).
- **は:** `LinkingParticle` (Topic).
- **から, が:** `ConjunctiveParticle` (Clause-separating).
- **も, より, まで, ぐらい:** `AdverbialParticle`.
