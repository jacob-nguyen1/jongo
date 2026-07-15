(async () => {
  try {
    let wasm = null;

    async function ensureWasm() {
      if (wasm) return wasm;
      const src = chrome.runtime.getURL("pkg/jongo.js");
      const mod = await import(src);
      await mod.default({ module_or_path: chrome.runtime.getURL("pkg/jongo_bg.wasm") });
      mod.content_start();
      wasm = mod;
      return wasm;
    }

    async function applyEnabled(enabled) {
      if (enabled) {
        const mod = await ensureWasm();
        mod.set_enabled(true);
      } else if (wasm) {
        wasm.set_enabled(false);
      }
    }

    let { enabled } = await chrome.storage.local.get("enabled");
    if (enabled === undefined) {
      enabled = true;
      await chrome.storage.local.set({ enabled: true });
    }
    await applyEnabled(enabled);

    chrome.storage.onChanged.addListener((changes, area) => {
      if (area === "local" && changes.enabled !== undefined) {
        applyEnabled(changes.enabled.newValue ?? true);
      }
    });
  } catch (e) {
    console.error("Jongo:", e);
  }
})();
