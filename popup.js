const toggle = document.getElementById("toggle");
const status = document.getElementById("status");

function updateStatus(enabled) {
  toggle.checked = enabled;
  status.textContent = enabled ? "Enabled" : "Disabled";
  status.className = enabled ? "status on" : "status off";
}

toggle.addEventListener("change", async () => {
  const enabled = toggle.checked;
  await chrome.storage.local.set({ enabled });
  updateStatus(enabled);
});

const llmUrlInput = document.getElementById("llm-url");
const llmKeyInput = document.getElementById("llm-key");
const saveBtn = document.getElementById("save-config");
const saveStatus = document.getElementById("save-status");

saveBtn.addEventListener("click", async () => {
  await chrome.storage.local.set({
    llmUrl: llmUrlInput.value.trim(),
    llmKey: llmKeyInput.value.trim()
  });
  saveStatus.style.opacity = "1";
  setTimeout(() => {
    saveStatus.style.opacity = "0";
  }, 2000);
});

(async () => {
  const data = await chrome.storage.local.get(["enabled", "llmUrl", "llmKey"]);
  updateStatus(data.enabled !== false); // default true
  if (data.llmUrl) llmUrlInput.value = data.llmUrl;
  if (data.llmKey) llmKeyInput.value = data.llmKey;
})();
