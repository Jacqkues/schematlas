// The parent reveals the live canvas only after WASM has rendered real nodes.
const observer = new MutationObserver(() => {
  if (!document.querySelector('.entity-node')) return;
  observer.disconnect();
  window.parent.postMessage({ type: 'schematlas:ready' }, window.location.origin);
});
observer.observe(document.documentElement, { childList: true, subtree: true });
