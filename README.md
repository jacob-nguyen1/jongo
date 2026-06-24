# Jongo

A browser extension for Japanese grammatical analysis. Jongo allows users to hover over Japanese text on any webpage, select sentences, and perform morphological analysis powered by Lindera (IPAdic dictionary).

## Build

```sh
cargo install wasm-pack && wasm-pack build --target web
```

## Repository

https://github.com/jacob-nguyen1/jongo

## Run

Load the build output into your browser:

- **Chrome**: Go to `chrome://extensions`, enable **Developer mode**, click **Load unpacked**, and select this directory.
- **Firefox**: Go to `about:debugging#/runtime/this-firefox`, click **Load Temporary Add-on...**, and select `manifest.json`.

Hold **Shift** and hover over a Japanese sentence, then click the **jong** button to analyze.



