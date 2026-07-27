async function syncBadge(enabled) {
  await chrome.action.setBadgeText({ text: enabled ? "" : "OFF" });
  await chrome.action.setTitle({
    title: enabled ? "Jongo (on)" : "Jongo (off)",
  });
}

chrome.runtime.onInstalled.addListener(async () => {
  await chrome.storage.local.set({ enabled: true });
  await syncBadge(true);
});

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && changes.enabled !== undefined) {
    syncBadge(changes.enabled.newValue ?? true);
  }
});