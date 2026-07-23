(async () => {
  try {
    const { enabled = true, darkMode = false, furigana = true, fontSize = 20, tooltips = true } = await chrome.storage.local.get([
      "enabled",
      "darkMode",
      "furigana",
      "fontSize",
      "tooltips"
    ]);

    const src = chrome.runtime.getURL("pkg/jongo.js");
    const { default: init, content_start, set_enabled, set_dark_mode, set_furigana, set_font_size, set_tooltips } = await import(src);
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

    // Expose settings fetchers on globalThis
    globalThis.__jongo_save_setting = (key, val) => {
      chrome.storage.local.set({[key]: val});
    };
    globalThis.__jongo_load_setting = async (key) => {
      const data = await chrome.storage.local.get(key);
      return data[key];
    };

    // Always start; gate inside Rust
    content_start();
    if (!enabled) set_enabled(false);
    set_dark_mode(!!darkMode);
    set_furigana(!!furigana);
    set_font_size(Number(fontSize) || 20);
    set_tooltips(tooltips !== false);

    chrome.storage.onChanged.addListener((changes, area) => {
      if (area !== "local") return;
      if (changes.enabled !== undefined) {
        set_enabled(changes.enabled.newValue ?? true);
      }
      if (changes.darkMode !== undefined) {
        set_dark_mode(changes.darkMode.newValue ?? false);
      }
      if (changes.furigana !== undefined) {
        set_furigana(changes.furigana.newValue ?? true);
      }
      if (changes.fontSize !== undefined) {
        set_font_size(Number(changes.fontSize.newValue) || 20);
      }
      if (changes.tooltips !== undefined) {
        set_tooltips(changes.tooltips.newValue !== false);
      }
    });
  } catch (e) {
    console.error("Jongo:", e);
  }
})();
