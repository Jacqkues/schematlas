// Keep worker lifetime bounded to one layout, including errors and source changes.
export function layoutInWorker(input, signal) {
  return new Promise((resolve, reject) => {
    if (signal.aborted) return reject(new Error('Layout cancelled.'));
    const worker = new Worker('/demo/layout-worker-loader.js');
    let timeout;
    const finish = (error, value) => {
      clearTimeout(timeout);
      signal.removeEventListener('abort', cancel);
      worker.terminate();
      if (error) reject(error);
      else resolve(value);
    };
    const cancel = () => finish(new Error('Layout cancelled.'));
    signal.addEventListener('abort', cancel, { once: true });
    timeout = setTimeout(
      () => finish(new Error('Layout took too long. Try arranging a smaller source.')),
      30000,
    );
    worker.onerror = () => finish(new Error('Could not load graph layout. Try again.'));
    worker.onmessage = ({ data }) => {
      if (data === 'ready') worker.postMessage(input);
      else finish(null, data);
    };
  });
}
