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

    window.__jongo_fetch_llm = async (prompt) => {
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
  } catch (e) {
    console.error("Jongo:", e);
  }
})();
