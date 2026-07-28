# Jongo

A browser extension for Japanese sentence analysis. Hover over Japanese text on any webpage, hold Shift, and click "jong" to open a morphological/grammatical analysis window powered by a manual rule-based parser (built on Lindera / IPADIC), the JMdict and JMnedict dictionaries, and optional LLM disambiguation.

## Repository

https://github.com/jacob-nguyen1/jongo

## Quickest path to try it (no build required)

The `prebuilt/` directory contains a ready-to-load extension.

1. Open Chrome → `chrome://extensions`
2. Enable **Developer mode** (top-right toggle)
3. Click **Load unpacked** and select the `prebuilt/` directory
4. Visit any webpage with Japanese text (e.g. https://ja.wikipedia.org)
5. Hold **Shift** and hover over a sentence — the "jong" button appears; click it to open the analysis window

## Building from source

Requirements:
- Rust (stable) — install via https://rustup.rs
- `wasm-pack` — install via `cargo install wasm-pack`
- Internet access (build.rs downloads JMdict/JMnedict dictionary data on first build, roughly 15 MB compressed, from GitHub)

Build steps (run from the `source/` directory inside this zip):

```sh
wasm-pack build --target web --release
```
