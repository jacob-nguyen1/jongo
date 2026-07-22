(async () => {
  try {
    const { enabled = true, darkMode = false } = await chrome.storage.local.get([
      "enabled",
      "darkMode",
    ]);

    const src = chrome.runtime.getURL("pkg/jongo.js");
    const { default: init, content_start, set_enabled, set_dark_mode } = await import(src);
    await init({ module_or_path: chrome.runtime.getURL("pkg/jongo_bg.wasm") });

    // Expose fetcher on globalThis so Rust can read it via js_sys::global()
    globalThis.__jongo_fetch_llm = async (prompt) => {
      const data = await chrome.storage.local.get(["llmUrl", "llmKey"]);
      if (!data.llmUrl || !data.llmKey) {
        console.error("Jongo: LLM URL or Key is missing. Configure in popup.");
        return null;
      }
      
      const res = await fetch(data.llmUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Authorization": `Bearer ${data.llmKey}`
        },
        body: JSON.stringify({
          model: "gemma-4-31b-it",
          messages: [{ role: "user", content: prompt }],
          temperature: 0.1
        })
      });
      
      const json = await res.json();
      return json.choices?.[0]?.message?.content || null;
    };

    globalThis.__jongo_set_dark_mode = async (val) => {
      await chrome.storage.local.set({ darkMode: !!val });
    };

    // Always start; gate inside Rust
    content_start();
    if (!enabled) set_enabled(false);
    set_dark_mode(!!darkMode);

    chrome.storage.onChanged.addListener((changes, area) => {
      if (area !== "local") return;
      if (changes.enabled !== undefined) {
        set_enabled(changes.enabled.newValue ?? true);
      }
      if (changes.darkMode !== undefined) {
        set_dark_mode(changes.darkMode.newValue ?? false);
      }
    });
  } catch (e) {
    console.error("Jongo:", e);
  }
})();
