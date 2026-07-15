async function syncBadge(enabled) {
  await chrome.action.setBadgeText({ text: enabled ? "" : "OFF" });
  await chrome.action.setTitle({
    title: enabled ? "Jongo (on)" : "Jongo (off)",
  });
}

chrome.runtime.onInstalled.addListener(async (details) => {
  if (details.reason === "install") {
    await chrome.storage.local.set({ enabled: true });
  }
  const { enabled = true } = await chrome.storage.local.get("enabled");
  await syncBadge(enabled);
});

chrome.runtime.onStartup.addListener(async () => {
  const { enabled = true } = await chrome.storage.local.get("enabled");
  await syncBadge(enabled);
});

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && changes.enabled !== undefined) {
    syncBadge(changes.enabled.newValue ?? true);
  }
});
