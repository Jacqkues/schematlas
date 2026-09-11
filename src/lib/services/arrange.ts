import type { CanvasGroup, Graph, Position } from '$lib/types';

export interface LayoutJob {
  result: Promise<Record<string, Position>>;
  cancel: () => void;
}

/** Runs smartLayout off the main thread. cancel() terminates the worker; the promise then never settles. */
export function arrangeInWorker(graph: Graph, groups: CanvasGroup[]): LayoutJob {
  const worker = new Worker(new URL('./layout.worker.ts', import.meta.url), { type: 'module' });
  const result = new Promise<Record<string, Position>>((resolve, reject) => {
    worker.onmessage = ({ data }) => {
      worker.terminate();
      if (data.error) reject(new Error(data.error));
      else resolve(data.positions);
    };
    worker.onerror = () => {
      worker.terminate();
      reject(new Error('Could not arrange the graph. Try again.'));
    };
    worker.postMessage({ graph, groups });
  });
  return { result, cancel: () => worker.terminate() };
}
