import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const sample = JSON.parse(readFileSync(new URL("../demo-ui/src/sample.json", import.meta.url), "utf8")).sources[0];
const tables = new Map(sample.graph.entities.map(table => [table.id, table]));
assert.equal(tables.size, 12);
assert.equal(sample.groups.length, 4);
const grouped = sample.groups.flatMap(group => group.nodeIds);
assert.equal(new Set(grouped).size, tables.size, "Every table belongs to a domain");
assert.equal(grouped.length, tables.size, "Domains do not share tables");
for (const table of tables.values()) {
  assert.ok(sample.positions[table.id], `Missing predefined position: ${table.name}`);
}
for (const relation of sample.graph.relations) {
  const source = tables.get(relation.source)?.fields.find(field => field.name === relation.sourceField);
  const target = tables.get(relation.target)?.fields.find(field => field.name === relation.targetField);
  assert.ok(source && target, `Invalid relationship: ${relation.id}`);
  assert.equal(source.dataType, target.dataType, `Incompatible foreign key: ${relation.id}`);
  assert.ok(target.primaryKey, `Foreign key must reference a primary key: ${relation.id}`);
}
for (const [index, table] of sample.graph.entities.entries()) {
  const a = sample.positions[table.id];
  const height = 106 + Math.min(table.fields.length, 9) * 29;
  for (const other of sample.graph.entities.slice(index + 1)) {
    const b = sample.positions[other.id];
    const otherHeight = 106 + Math.min(other.fields.length, 9) * 29;
    assert.ok(a.x + 284 <= b.x || b.x + 284 <= a.x || a.y + height <= b.y || b.y + otherHeight <= a.y, `Overlapping tables: ${table.name}, ${other.name}`);
  }
}

import { chromium } from "playwright-core";

const base = process.env.DEMO_TEST_URL ?? "http://localhost:3000";
const browser = await chromium.launch({ channel: "chrome", headless: true });
const errors = [];
const page = await browser.newPage({ viewport: { width: 1440, height: 1100 }, reducedMotion: "reduce" });
page.on("pageerror", error => errors.push(error.message));
const pause = () => page.waitForTimeout(450);
try {
  await page.goto(base);
  assert.equal(await page.locator(".demo-iframe").count(), 0, "WASM is deferred below the hero");
  await page.locator(".workspace-demo").scrollIntoViewIfNeeded();
  await page.locator(".demo-iframe.is-ready").waitFor();
  const frame = page.frameLocator(".demo-iframe");
  const graph = frame.getByLabel("Interactive schema graph");
  const orders = frame.locator(".entity-node").filter({ has: frame.locator("strong", { hasText: /^orders$/ }) });
  assert.equal(await frame.locator(".entity-node").count(), 12);
  assert.equal(await frame.getByRole("button", { name: /^Move group / }).count(), 4);
  await orders.click();
  assert.equal(await frame.locator(".entity-node[data-selected]").count(), 1);
  assert.ok(await frame.locator('.entity-node[data-state="dimmed"]').count() > 0);
  assert.equal(await frame.locator(".inspector").count(), 0, "Selection does not force the inspector open");
  await frame.getByRole("button", { name: "Inspect main.orders", exact: true }).click();
  assert.ok((await frame.locator(".inspector").innerText()).includes("customer_id"));
  await frame.getByRole("button", { name: "Close inspector" }).click();

  const position = () => orders.evaluate(el => el.parentElement.style.transform);
  const original = await position();
  const box = await orders.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + 20);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 65, box.y + 60, { steps: 8 });
  await page.mouse.up();
  await pause();
  const moved = await position();
  assert.notEqual(moved, original, "Dragging changes the node's graph position");

  const beforeZoom = await graph.evaluate(el => el.style.backgroundSize);
  await frame.getByRole("button", { name: "Zoom in", exact: true }).click();
  await pause();
  assert.notEqual(await graph.evaluate(el => el.style.backgroundSize), beforeZoom);
  const viewport = await graph.boundingBox();
  const beforeWheel = await graph.evaluate(el => el.style.backgroundSize);
  await page.mouse.move(viewport.x + viewport.width / 2, viewport.y + viewport.height / 2);
  await page.mouse.wheel(0, -120);
  await pause();
  assert.notEqual(await graph.evaluate(el => el.style.backgroundSize), beforeWheel, "Mouse wheel zoom works without modifier keys");
  const beforePan = await graph.evaluate(el => el.style.backgroundPosition);
  await page.mouse.move(viewport.x + 25, viewport.y + 80);
  await page.mouse.down({ button: "middle" });
  await page.mouse.move(viewport.x + 85, viewport.y + 100, { steps: 8 });
  await page.mouse.up({ button: "middle" });
  await pause();
  assert.notEqual(await graph.evaluate(el => el.style.backgroundPosition), beforePan);

  await frame.getByRole("button", { name: "OpenAPI", exact: true }).click();
  await frame.getByRole("button", { name: "Fit graph to screen" }).click();
  await pause();
  assert.equal(await frame.locator(".entity-node").count(), 9);
  const apiPositions = () => frame.locator(".entity-node").evaluateAll(nodes => Object.fromEntries(nodes.map(node => {
    const transform = new DOMMatrix(node.parentElement.style.transform);
    return [node.querySelector("strong").textContent, { x: transform.e, y: transform.f }];
  })));
  const apiLayout = await apiPositions();
  for (const [route, model, nested] of [
    ["/products", "Product"], ["/customers/{id}", "Customer", "Address"], ["/orders", "Order", "OrderItem"],
  ]) {
    assert.equal(apiLayout[route].y, apiLayout[model].y, "Route aligns with its model");
    assert.ok(apiLayout[route].x + 284 < apiLayout[model].x);
    if (nested) {
      assert.equal(apiLayout[model].y, apiLayout[nested].y);
      assert.ok(apiLayout[model].x + 284 < apiLayout[nested].x);
    }
  }
  assert.ok(apiLayout.Error.y > apiLayout.Order.y + 251, "Shared errors sit below route rows");
  const apiPaths = await frame.locator(".edge-path").evaluateAll(edges => edges.map(edge => edge.getAttribute("d")));
  assert.equal(apiPaths.length, 9);
  for (const path of apiPaths) {
    assert.match(path, /^M/);
    assert.ok(path.includes("H") && path.includes("V") && !/[CQ]|NaN|Infinity/.test(path));
  }
  await frame.getByRole("button", { name: "Auto layout", exact: true }).click();
  await page.waitForFunction(() => !document.querySelector("iframe").contentDocument.querySelector('button[aria-label="Auto layout"]').disabled);
  assert.deepEqual(await apiPositions(), apiLayout, "API worker layout restores aligned route rows");

  await frame.getByLabel("Inspect a node", { exact: true }).selectOption({ index: 1 });
  await frame.locator(".inspector").waitFor();
  await frame.getByRole("button", { name: "Close inspector" }).click();
  await frame.getByRole("button", { name: "Database", exact: true }).click();
  assert.equal(await position(), moved, "Source switching preserves in-memory layout edits");
  await frame.getByRole("button", { name: "Reset demo", exact: true }).click();
  assert.equal(await position(), original, "Reset restores the original layout");
  await frame.getByRole("button", { name: "OpenAPI", exact: true }).click();
  assert.deepEqual(await apiPositions(), apiLayout, "Reset restores aligned API rows");
  await frame.getByRole("button", { name: "Database", exact: true }).click();
  await frame.getByLabel("Search sample schema").fill("customers");
  await pause();
  assert.equal(await frame.locator(".entity-node").count(), 1);
  assert.ok((await frame.locator(".entity-node").innerText()).includes("customers"));
  await frame.getByLabel("Search sample schema").fill("");
  await frame.getByRole("button", { name: "Auto layout", exact: true }).click();
  await page.waitForFunction(() => {
    const doc = document.querySelector("iframe")?.contentDocument;
    return doc && !doc.querySelector('button[aria-label="Auto layout"]').disabled;
  });
  assert.equal(await frame.getByRole("alert").count(), 0, "Worker layout succeeds");
  await frame.getByRole("button", { name: "Reset demo", exact: true }).click();
  await page.locator(".workspace-demo").screenshot({ path: "/tmp/schematlas-demo-desktop.png" });

  await page.setViewportSize({ width: 390, height: 844 });
  await pause();
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  assert.equal(await frame.locator("html").evaluate(el => el.scrollWidth > innerWidth), false);
  await frame.getByRole("button", { name: "OpenAPI", exact: true }).click();
  await frame.getByLabel("Inspect a node", { exact: true }).selectOption({ index: 1 });
  await frame.locator(".inspector").waitFor();
  await frame.getByRole("button", { name: "Close inspector" }).click();
  await frame.getByRole("button", { name: "Database", exact: true }).click();
  await page.locator(".workspace-demo").screenshot({ path: "/tmp/schematlas-demo-mobile.png" });
  assert.deepEqual(errors, [], "No browser runtime errors");
  console.log("PASS: deferred loading, selection, inspection, drag, zoom, middle-button pan, API switching, reset, search, worker layout, mobile controls and overflow.");

  const failure = await browser.newPage({ viewport: { width: 1440, height: 1000 }, reducedMotion: "reduce" });
  await failure.route("**/demo/*.wasm", route => route.abort());
  await failure.goto(`${base}/#workspace`);
  await failure.getByRole("button", { name: "Retry workspace" }).waitFor({ timeout: 35_000 });
  await failure.unroute("**/demo/*.wasm");
  await failure.getByRole("button", { name: "Retry workspace" }).click();
  await failure.locator(".demo-iframe.is-ready").waitFor();
  console.log("PASS: blocked WASM has a usable fallback and retry recovers.");
} finally {
  await browser.close();
}
