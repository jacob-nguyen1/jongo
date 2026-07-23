# AGENTS.md

## Cursor Cloud specific instructions

Jongo is a single Rust product with two build outputs (see `README.md`):
- A **Manifest V3 browser extension** (primary product): WASM built from `src/lib.rs` via `wasm-pack`, loaded through `content_shim.js` + `manifest.json`.
- An **optional native CLI** (`src/main.rs` → `grammar::grammar()`) for terminal-based grammar analysis.

There is no backend, database, or network service; the IPAdic and JMdict dictionaries are embedded at compile time.

### Toolchain
- Requires Rust **≥ 1.85** (crate uses `edition = "2024"`). The update script sets the default toolchain to `stable` and adds the `wasm32-unknown-unknown` target and `wasm-pack`.

### Build / test / run
- **Build extension:** `wasm-pack build --target web` → outputs to `pkg/` (`jongo.js`, `jongo_bg.wasm`). `pkg/` is git-ignored (via `pkg/.gitignore`) and must be rebuilt before loading the extension.
- **Test:** `cargo test` (unit tests live in `src/jmdict.rs`).
- **Native CLI:** `cargo run`. It is interactive; drive it non-interactively with piped stdin, e.g. `printf '1\n2\n5\n' | cargo run` (1 = saved sample text, 2 = raw English breakdown, 5 = exit).

### Running the browser extension (non-obvious)
- The `pkg/` directory must exist (run `wasm-pack build --target web`) before loading the extension in Chrome via `chrome://extensions` → Developer mode → Load unpacked → select the repo root.
- The content script matches `<all_urls>`. To test on a local `file://` page you must enable "Allow access to file URLs" for the extension; otherwise serve the test page over HTTP (e.g. `python3 -m http.server`) and open `http://localhost:<port>/`.
- Usage: hold **Shift** and hover over a Japanese sentence to spawn the `jong` prompt, then click `jong` to show the token-by-token analysis popup.
- The first analysis is slow: the embedded Lindera IPAdic dictionary is lazily initialized on first tokenize.
