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

(async () => {
  const { enabled = true } = await chrome.storage.local.get("enabled");
  updateStatus(enabled);
})();
