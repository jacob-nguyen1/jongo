# Jongo Parser — Known Failures & TODOs

## sentence.rs

### Actionable Fixes
- **[F1] Te-form predicate chain split:** `買ってくれた` splits incorrectly. **Fix:** Add word bank guard (`くれる、もらう、あげる、いる、しまう、おく、みる、いく、くる、ある`). Keep tokens separate in `grammar.rs`.
- **[F2] Eager modifier attachment:** Modifier attaches to wrong noun (e.g. `東京` instead of `電車`). **Fix:** Add `is_head: bool` (default `true`) to `Chunk`. When `の` captures a noun, mark it `is_head: false`. Skip non-head chunks in modifier attachment.
- **[F3] `の` pops wrong chunk:** For Noun+Particle+の (e.g. `東京からの`), `の` pops `から` and orphans `東京`. **Fix:** If preceding chunk is a particle, keep popping until reaching a noun. Store particle on the noun chunk.
- **[F4] Copular quotation not detected:** `便利だ+と` doesn't trigger Quotation. **Fix:** Check if `と` is tagged `MarkingParticle, Quotation` instead of checking preceding POS.
- **[F6] Sentence-final explanatory `の`:** Dangles as standalone chunk. **Fix:** Lindera tags as `EndingParticle`. Handle at clause level with `ending_particle: Option<ProcToken>`. (Hardcode list: の, ね, よ, わ, な).
- **[F7] Limitation print gap:** Nested modifiers in `Limitation` chunk don't print. **Fix:** Recurse into limitation chunk's modifiers in `Sentence::print()`.

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
  - `Destination` (`へ/に`) vs `IndirectObject` (`に`) vs `Agent` (`に`)
- **Formal `より` ("from"):** Identical to comparison `より`, mislabeled as comparison. Low priority.

## grammar.rs

### Actionable Fixes
- **[G2] Arabic numeral counters:** `2つ目と3つ目` parsed incorrectly (`目と` as suru verb). Kanji numerals (`二つ目`) work. **Fix:** Pre-process string to convert arabic numerals to kanji in counter contexts before Lindera call.

### Teammate Responsibility
- **[G3] ConjugationFeatures:** Populate `is_negative`, `is_past`, `is_te_form` as tokens merge. `sentence.rs` will later check `token.conjugation.is_te_form`.

## Particle Ambiguity Findings (に, で, より, から)
- **に (Ni):** 6-way ambiguous (`MarkingParticle, General`). Defer all resolution to LLM.
- **で (De):** Conjunctive で (`親切で`) is `AuxiliaryVerb` (だ), not a particle. No collision with LocationAction/Means.
- **より (Yori):** Adverbial (`Adverb, General`) and Comparison (`Particle, MarkingParticle, General`) are distinct. Formal ("from") matches comparison.
- **から (Kara):** Temporal vs. Spatial is unresolvable (`Particle, MarkingParticle, General`). Defer to LLM.

## Lindera Subcategory Findings
- **と:** `MarkingParticle, Quotation` (Quotation), `MarkingParticle, General` (Accompaniment), `CoordinatingParticle` (Listing).
- **の, ね, よ, わ, な:** `EndingParticle` (Sentence-final).
- **の:** `Noun, Bound` (Nominalizing), `NormalizingParticle` (Structural).
- **は:** `LinkingParticle` (Topic).
- **から, が:** `ConjunctiveParticle` (Clause-separating).
- **も, より, まで, ぐらい:** `AdverbialParticle`.
