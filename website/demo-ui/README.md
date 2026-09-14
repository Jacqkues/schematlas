# Interactive Schematlas demo

This browser-only Leptos shell reuses the real Schematlas graph, relationship routing,
inspector, and background layout worker. It contains only synthetic Northstar Commerce
data. Changes live in memory, survive source switching, and clear on Reset or reload.
There is no Tauri IPC, database driver, HTTP query tool, agent runtime, or browser storage.

The graph, geometry, layout, types, icons, inspector, sample and base styles are copied
from `Jacqkues/schematlas`, revision `7d7b3de861b5aac30c180f3e2d053ba757a2a64c`.
They retain the Apache-2.0 license in LICENSE. The icon source notes its Lucide ISC origin.
Demo-specific code is main.rs, demo.css, index.html, ready.js, and the build configuration.
Asset URLs in the base stylesheet and worker client are adapted to `/demo/`.
The sample database has twelve tables and thirteen foreign-key relationships, with a
curated opening layout in four colored domain lanes: Customers, Ordering, Catalog, and
Fulfillment. Every node has a predefined position in `src/sample.json`; Reset restores
that arrangement. The original six-table fixture has been expanded with synthetic
segments, categories, suppliers, warehouses, shipments, and shipment items. The mouse wheel zooms the graph,
as in the desktop app; scroll outside the canvas to continue through the page.

The OpenAPI layout and right-angle routing are updated from revision
`80822a5` ([PR #16](https://github.com/Jacqkues/schematlas/pull/16)).
Routes align with their request/response models in rows; nested models follow to the
right, with shared models below. Both Reset and Auto layout restore this arrangement.
Database positions and domain groups are unchanged.

## Rebuild

Requires Rust with `wasm32-unknown-unknown`, Trunk 0.21.14, and the site's npm dependencies.
Run `npm run build:demo` from the site root after changing demo sources. Trunk writes
the browser artifacts into `public/demo/`; these are committed so ordinary site builds
do not need Rust. Then run the normal site build.

The landing page defers the iframe until the preview approaches the viewport and keeps
its poster visible until the WASM graph signals readiness. A timeout offers retry; the
full workspace link provides a larger view. The keyboard-accessible node selector gives
access to the same inspector without dragging or precise pointer interaction.
