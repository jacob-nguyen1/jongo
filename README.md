# Jongo

A browser extension for Japanese grammatical analysis. Jongo allows users to hover over Japanese text on any webpage, select sentences, and perform morphological analysis powered by Lindera (IPAdic dictionary). The extension displays a per-token breakdown of surface form, base form, and part of speech directly in-page.

Built with Rust compiled to WebAssembly for performance.

## Build

```
cargo install wasm-pack
wasm-pack build --target web
```

## Repository

https://github.com/jacob-nguyen1/jongo

## Run

1. Build the project using the command above.
2. Open your Chromium-based browser and navigate to `chrome://extensions`.
3. Enable **Developer mode**.
4. Click **Load unpacked** and select the `jongo/` project directory.
5. Navigate to any page with Japanese text. Hold **Shift** and move the mouse to highlight a sentence, then click the **jong** button to analyze it.
