// The Excalidraw editor, mounted by the Leptos UI on `.excalidraw` files. Built by esbuild into
// public/excalidraw/ (scripts/leptos.mjs) and loaded only when such a file is opened.
//
// The file is the source of truth: the editor reports the whole serialized scene after each
// change, and Rust writes it with compare-and-swap. Schematlas's own data rides along in
// `customData.schematlas` on elements and a top-level `schematlas` key, which Excalidraw ignores.
import { Excalidraw, serializeAsJSON } from '@excalidraw/excalidraw';
import '@excalidraw/excalidraw/index.css';
import './theme.css';
import { createElement, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';

window.EXCALIDRAW_ASSET_PATH = new URL('./', import.meta.url).href;

function parse(text) {
  try {
    const data = JSON.parse(text || '{}');
    return data && typeof data === 'object' ? data : {};
  } catch {
    return {};
  }
}

/** Sum of element versions: changes whenever any element changes, not when the view moves. */
function version(elements) {
  let sum = elements.length;
  for (const e of elements) sum += e.version;
  return sum;
}

/** The shape a selection stands for: a label selects its container. */
function selectedNode(elements, ids) {
  const keys = Object.keys(ids || {});
  if (keys.length !== 1) return null;
  const element = elements.find((e) => e.id === keys[0]);
  if (!element) return null;
  return element.containerId || element.id;
}

function Editor({ text, theme, onSave, ready }) {
  const [api, setApi] = useState(null);
  useEffect(() => {
    if (api) ready(api);
  }, [api]);
  const data = parse(text);
  return createElement(Excalidraw, {
    excalidrawAPI: setApi,
    theme,
    initialData: {
      elements: data.elements || [],
      // Transparent, so the app's canvas colour shows through in both themes.
      appState: { ...(data.appState || {}), theme, viewBackgroundColor: 'transparent' },
      files: data.files || {},
      scrollToContent: true,
    },
    UIOptions: {
      canvasActions: {
        loadScene: false,
        saveToActiveFile: false,
        export: false,
        changeViewBackgroundColor: false,
      },
    },
    onChange: onSave,
  });
}

/**
 * Mount an editor in `element`.
 * - `text`: the file's JSON.
 * - `onSave(text)`: the new file text, after the drawing settles.
 * - `onSelect(id | null)`: the single shape selected, if any.
 * Returns `{ load(text), setTheme(theme), unmount() }`.
 */
function mount(element, { text, theme = 'light', onSave, onSelect }) {
  const root = createRoot(element);
  let api = null;
  let current = text;
  let base = parse(text);
  let seen = null;
  let selected = null;
  let timer = null;
  let pending = null;
  let applying = false;

  const flush = () => {
    clearTimeout(timer);
    timer = null;
    if (!pending) return;
    const { elements, appState, files } = pending;
    pending = null;
    const scene = JSON.parse(serializeAsJSON(elements, appState, files, 'local'));
    // The file keeps the background it was drawn on (shown by excalidraw.com and exports).
    scene.appState.viewBackgroundColor = base.appState?.viewBackgroundColor ?? '#ffffff';
    // Keep what Schematlas stores beside the scene.
    if (base.schematlas !== undefined) scene.schematlas = base.schematlas;
    const next = JSON.stringify(scene, null, 2) + '\n';
    if (next !== current) {
      current = next;
      onSave(next);
    }
  };

  const save = (elements, appState, files) => {
    const node = selectedNode(elements, appState.selectedElementIds);
    if (node !== selected) {
      selected = node;
      onSelect?.(node);
    }
    const v = version(elements);
    if (seen === null) {
      seen = v;
      return;
    }
    if (v === seen || applying) {
      seen = v;
      return;
    }
    seen = v;
    pending = { elements, appState, files };
    clearTimeout(timer);
    timer = setTimeout(flush, 500);
  };

  root.render(createElement(Editor, { text, theme, onSave: save, ready: (a) => (api = a) }));

  return {
    /** Show a newer text of the file (written by an agent, an editor or git). */
    load(text) {
      if (text === current) return;
      current = text;
      base = parse(text);
      if (!api) return;
      clearTimeout(timer);
      pending = null;
      applying = true;
      api.updateScene({ elements: base.elements || [] });
      if (base.files) api.addFiles(Object.values(base.files));
      applying = false;
      seen = version(api.getSceneElements());
    },
    setTheme(theme) {
      api?.updateScene({ appState: { theme } });
    },
    unmount() {
      flush();
      root.unmount();
    },
  };
}

window.SchematlasExcalidraw = { mount };
window.dispatchEvent(new Event('schematlas-excalidraw'));
