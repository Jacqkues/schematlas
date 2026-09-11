import { smartLayout } from './smart-layout';
import type { Graph, CanvasGroup } from '$lib/types';
self.onmessage = (event: MessageEvent<{ graph: Graph; groups: CanvasGroup[] }>) => {
  try {
    self.postMessage({ positions: smartLayout(event.data.graph, event.data.groups) });
  } catch (error) {
    self.postMessage({ error: error instanceof Error ? error.message : 'Layout failed.' });
  }
};
