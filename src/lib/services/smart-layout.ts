import dagre from '@dagrejs/dagre';
import type { CanvasGroup, Graph, Position } from '$lib/types';
import { graphBounds, NODE_WIDTH, nodeHeight } from './layout';

type Block = { positions: Record<string, Position>; width: number; height: number };
/** Stable shelf packing keeps disconnected components and domains compact and separate. */
function pack(blocks: Block[], gap: number): Block {
  const sorted = [...blocks].sort((a, b) => b.height - a.height || b.width - a.width);
  const target = Math.max(
    1,
    ...blocks.map((b) => b.width),
    Math.sqrt(blocks.reduce((n, b) => n + (b.width + gap) * (b.height + gap), 0) * 1.5),
  );
  const positions: Record<string, Position> = {};
  let x = 0,
    y = 0,
    row = 0,
    width = 0;
  for (const block of sorted) {
    if (x && x + block.width > target) {
      y += row + gap;
      x = 0;
      row = 0;
    }
    for (const [id, p] of Object.entries(block.positions))
      positions[id] = { x: x + p.x, y: y + p.y };
    width = Math.max(width, x + block.width);
    row = Math.max(row, block.height);
    x += block.width + gap;
  }
  return { positions, width, height: y + row };
}
function connected(graph: Graph): Graph[] {
  const byId = new Map(graph.entities.map((e) => [e.id, e]));
  const adjacency = new Map(graph.entities.map((e) => [e.id, new Set<string>()]));
  for (const r of graph.relations) {
    adjacency.get(r.source)?.add(r.target);
    adjacency.get(r.target)?.add(r.source);
  }
  const seen = new Set<string>();
  const result: Graph[] = [];
  for (const e of graph.entities) {
    if (seen.has(e.id)) continue;
    const queue = [e.id];
    seen.add(e.id);
    for (let i = 0; i < queue.length; i++)
      for (const next of adjacency.get(queue[i]) ?? []) {
        if (byId.has(next) && !seen.has(next)) {
          seen.add(next);
          queue.push(next);
        }
      }
    const ids = new Set(queue);
    result.push({
      entities: queue.map((id) => byId.get(id)!),
      relations: graph.relations.filter((r) => ids.has(r.source) && ids.has(r.target)),
      warnings: [],
    });
  }
  return result;
}
function layered(graph: Graph, direction: 'LR' | 'TB'): Block {
  const g = new dagre.graphlib.Graph()
    .setGraph({ rankdir: direction, nodesep: 56, ranksep: 110 })
    .setDefaultEdgeLabel(() => ({}));
  for (const e of graph.entities)
    g.setNode(e.id, { width: NODE_WIDTH, height: nodeHeight(e.fields.length) });
  for (const r of graph.relations) if (r.source !== r.target) g.setEdge(r.source, r.target);
  dagre.layout(g);
  const positions = Object.fromEntries(
    graph.entities.map((e) => {
      const p = g.node(e.id);
      return [e.id, { x: p.x - NODE_WIDTH / 2, y: p.y - nodeHeight(e.fields.length) / 2 }];
    }),
  );
  const bounds = graphBounds(graph, positions)!;
  return {
    positions: Object.fromEntries(
      Object.entries(positions).map(([id, p]) => [id, { x: p.x - bounds.x, y: p.y - bounds.y }]),
    ),
    width: bounds.width,
    height: bounds.height,
  };
}
/** Dense/long-chain domains use a neighbor-ordered grid when ranks waste substantial space. */
function compactGrid(graph: Graph): Block {
  const degree = new Map(graph.entities.map((e) => [e.id, 0]));
  for (const r of graph.relations) {
    degree.set(r.source, (degree.get(r.source) ?? 0) + 1);
    degree.set(r.target, (degree.get(r.target) ?? 0) + 1);
  }
  const ordered = [...graph.entities].sort(
    (a, b) => degree.get(b.id)! - degree.get(a.id)! || a.id.localeCompare(b.id),
  );
  // The connected traversal places direct neighbors next to one another.
  const walk = connected({ ...graph, entities: ordered }).flatMap((part) => part.entities);
  const candidates: Block[] = [];
  for (let columns = 1; columns <= Math.ceil(Math.sqrt(walk.length)) * 2; columns++) {
    const positions: Record<string, Position> = {};
    let y = 0;
    let width = 0;
    for (let start = 0; start < walk.length; start += columns) {
      const row = walk.slice(start, start + columns);
      row.forEach((e, i) => {
        positions[e.id] = { x: i * (NODE_WIDTH + 100), y };
      });
      width = Math.max(width, row.length * (NODE_WIDTH + 100) - 100);
      y += Math.max(...row.map((e) => nodeHeight(e.fields.length))) + 80;
    }
    candidates.push({ positions, width, height: y - 80 });
  }
  const cost = (b: Block) =>
    b.width * b.height * (1 + Math.abs(Math.log(b.width / b.height / 1.4)) * 0.5);
  return candidates.sort((a, b) => cost(a) - cost(b))[0];
}
export function smartLayout(graph: Graph, groups: CanvasGroup[]): Record<string, Position> {
  if (!graph.entities.length) return {};
  // Overlapping groups share a region, avoiding invalid overlays spanning unrelated domains.
  const owner = new Map<string, string>();
  for (const group of groups) {
    const overlaps = new Set(
      group.nodeIds.map((id) => owner.get(id)).filter((id): id is string => !!id),
    );
    const region = [...overlaps][0] ?? `group:${group.id}`;
    for (const [id, previous] of owner) if (overlaps.has(previous)) owner.set(id, region);
    for (const id of group.nodeIds) owner.set(id, region);
  }
  const regions = new Map<string, Set<string>>();
  for (const e of graph.entities) {
    const key = owner.get(e.id) ?? `schema:${e.namespace}`;
    const ids = regions.get(key) ?? new Set<string>();
    ids.add(e.id);
    regions.set(key, ids);
  }
  const blocks = [...regions.values()].map((ids) => {
    const domain = {
      entities: graph.entities.filter((e) => ids.has(e.id)),
      relations: graph.relations.filter((r) => ids.has(r.source) && ids.has(r.target)),
      warnings: [],
    };
    return pack(
      connected(domain).map((component) => {
        const lr = layered(component, 'LR'),
          tb = layered(component, 'TB');
        const score = (b: Block) =>
          b.width * b.height * (1 + Math.abs(Math.log(b.width / b.height / 1.5)) * 0.35);
        const ranked = score(lr) <= score(tb) ? lr : tb;
        const compact = compactGrid(component);
        return score(ranked) > score(compact) * 1.35 ? compact : ranked;
      }),
      80,
    );
  });
  return Object.fromEntries(
    Object.entries(pack(blocks, 160).positions).map(([id, p]) => [
      id,
      { x: p.x + 60, y: p.y + 80 },
    ]),
  );
}
