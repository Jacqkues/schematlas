// Trunk emits the worker binding and WASM under stable names.
importScripts('./layout-worker.js');
wasm_bindgen('./layout-worker_bg.wasm').catch(() => {
  self.postMessage(JSON.stringify({ Err: 'Could not initialize graph layout.' }));
});
