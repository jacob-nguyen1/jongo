(async () => {
  try {
    const { enabled = true } = await chrome.storage.local.get("enabled");

    const src = chrome.runtime.getURL("pkg/jongo.js");
    const { default: init, content_start, set_enabled } = await import(src);
    await init({ module_or_path: chrome.runtime.getURL("pkg/jongo_bg.wasm") });

    // Always start; gate inside Rust
    content_start();
    if (!enabled) set_enabled(false);

    chrome.storage.onChanged.addListener((changes, area) => {
      if (area === "local" && changes.enabled !== undefined) {
        set_enabled(changes.enabled.newValue ?? true);
      }
    });
  } catch (e) {
    console.error("Jongo:", e);
  }
})();
