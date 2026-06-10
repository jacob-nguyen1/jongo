(async () => {
  const src = chrome.runtime.getURL('pkg/jongo.js');
  const { default: init, content_start } = await import(src);
  await init({ module_or_path: chrome.runtime.getURL('pkg/jongo_bg.wasm') });
  content_start();
})();
