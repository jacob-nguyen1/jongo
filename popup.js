const toggle = document.getElementById("toggle");
const status = document.getElementById("status");
const settingsBtn = document.getElementById("settings-btn");
const mainView = document.getElementById("main-view");
const settingsView = document.getElementById("settings-view");
const settingsBack = document.getElementById("settings-back");
const darkToggle = document.getElementById("dark-toggle");
const furiganaToggle = document.getElementById("furigana-toggle");
const tooltipToggle = document.getElementById("tooltip-toggle");
const aiSetupBtn = document.getElementById("ai-setup-btn");
const aiConfig = document.getElementById("ai-config");
const llmUrlInput = document.getElementById("llm-url");
const llmKeyInput = document.getElementById("llm-key");
const saveBtn = document.getElementById("save-config");
const saveStatus = document.getElementById("save-status");

function updateStatus(enabled) {
  toggle.checked = enabled;
  status.textContent = enabled ? "Enabled" : "Disabled";
  status.className = enabled ? "status on" : "status off";
}

function applyDarkMode(on) {
  if (on) {
    document.body.setAttribute("data-jong-dark", "1");
  } else {
    document.body.removeAttribute("data-jong-dark");
  }
}

function showMainView() {
  mainView.classList.remove("hidden");
  settingsView.classList.add("hidden");
  settingsBtn.classList.remove("active");
}

function showSettingsView() {
  mainView.classList.add("hidden");
  settingsView.classList.remove("hidden");
  settingsBtn.classList.add("active");
}

settingsBtn.addEventListener("click", () => {
  if (settingsView.classList.contains("hidden")) {
    showSettingsView();
  } else {
    showMainView();
  }
});

settingsBack.addEventListener("click", showMainView);

toggle.addEventListener("change", async () => {
  const enabled = toggle.checked;
  await chrome.storage.local.set({ enabled });
  updateStatus(enabled);
});

darkToggle.addEventListener("change", async () => {
  const on = darkToggle.checked;
  applyDarkMode(on);
  await chrome.storage.local.set({ darkMode: on });
});

furiganaToggle.addEventListener("change", async () => {
  await chrome.storage.local.set({ furigana: furiganaToggle.checked });
});

tooltipToggle.addEventListener("change", async () => {
  await chrome.storage.local.set({ tooltips: tooltipToggle.checked });
});

aiSetupBtn.addEventListener("click", () => {
  const open = aiConfig.classList.toggle("hidden");
  aiSetupBtn.classList.toggle("open", !open);
});

saveBtn.addEventListener("click", async () => {
  await chrome.storage.local.set({
    llmUrl: llmUrlInput.value.trim(),
    llmKey: llmKeyInput.value.trim(),
  });
  saveStatus.style.opacity = "1";
  setTimeout(() => {
    saveStatus.style.opacity = "0";
  }, 2000);
});

(async () => {
  const data = await chrome.storage.local.get([
    "enabled",
    "darkMode",
    "furigana",
    "tooltips",
    "llmUrl",
    "llmKey",
  ]);
  updateStatus(data.enabled !== false);
  const dark = !!data.darkMode;
  darkToggle.checked = dark;
  applyDarkMode(dark);
  furiganaToggle.checked = data.furigana !== false;
  tooltipToggle.checked = data.tooltips !== false;
  if (data.llmUrl) llmUrlInput.value = data.llmUrl;
  if (data.llmKey) llmKeyInput.value = data.llmKey;

  chrome.storage.onChanged.addListener((changes, area) => {
    if (area !== "local" || changes.darkMode === undefined) return;
    const on = changes.darkMode.newValue ?? false;
    darkToggle.checked = on;
    applyDarkMode(on);
  });
})();
