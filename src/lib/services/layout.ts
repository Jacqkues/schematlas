import dagre from '@dagrejs/dagre';
import type { Graph, Position } from '$lib/types';
export const NODE_WIDTH = 284;
export function nodeHeight(fieldCount: number): number {
  return 106 + Math.min(fieldCount, 9) * 29 + (fieldCount > 9 ? 29 : 0);
}
export function layoutGraph(graph: Graph): Record<string, Position> {
  if (!graph.entities.length) return {};
  const layout = new dagre.graphlib.Graph({ multigraph: true })
    .setGraph({ rankdir: 'LR', nodesep: 48, ranksep: 70, marginx: 40, marginy: 40 })
    .setDefaultEdgeLabel(() => ({}));
  graph.entities.forEach((e) =>
    layout.setNode(e.id, { width: NODE_WIDTH, height: nodeHeight(e.fields.length) }),
  );
  graph.relations.forEach((r) => layout.setEdge(r.source, r.target, {}, r.id));
  dagre.layout(layout);
  return Object.fromEntries(
    graph.entities.map((e) => {
      const n = layout.node(e.id);
      return [e.id, { x: n.x - NODE_WIDTH / 2, y: n.y - nodeHeight(e.fields.length) / 2 }];
    }),
  );
}
export function matchingIds(graph: Graph, query: string): Set<string> {
  const q = query.trim().toLocaleLowerCase();
  return new Set(
    graph.entities
      .filter(
        (e) =>
          !q ||
          [e.name, e.namespace, e.method, ...e.fields.map((f) => f.name)].some((v) =>
            v?.toLocaleLowerCase().includes(q),
          ),
      )
      .map((e) => e.id),
  );
}

export function schemaNamespaces(graph: Graph): { name: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const entity of graph.entities)
    counts.set(entity.namespace, (counts.get(entity.namespace) ?? 0) + 1);
  return [...counts]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, count]) => ({ name, count }));
}
export function filterGraph(graph: Graph, namespaces: string[], related: boolean): Graph {
  if (!namespaces.length) return graph;
  const selected = new Set(
    graph.entities.filter((e) => namespaces.includes(e.namespace)).map((e) => e.id),
  );
  const visible = new Set(selected);
  if (related)
    for (const edge of graph.relations) {
      if (selected.has(edge.source) || selected.has(edge.target)) {
        visible.add(edge.source);
        visible.add(edge.target);
      }
    }
  return {
    entities: graph.entities.filter((e) => visible.has(e.id)),
    relations: graph.relations.filter((r) => visible.has(r.source) && visible.has(r.target)),
    warnings: graph.warnings,
  };
}

/** Overlays are visual bounds, not SvelteFlow parents: nodes retain absolute coordinates. */
export function groupBounds(
  groups: import('$lib/types').CanvasGroup[],
  graph: Graph,
  positions: Record<string, Position>,
) {
  const entities = new Map(graph.entities.map((entity) => [entity.id, entity]));
  return groups.flatMap((group) => {
    const members = [...new Set(group.nodeIds)].flatMap((id) => {
      const entity = entities.get(id);
      return entity && positions[id] ? [entity] : [];
    });
    if (!members.length) return [];
    const x = Math.min(...members.map((e) => positions[e.id].x)) - 28;
    const y = Math.min(...members.map((e) => positions[e.id].y)) - 48;
    const right = Math.max(...members.map((e) => positions[e.id].x + NODE_WIDTH)) + 28;
    const bottom =
      Math.max(...members.map((e) => positions[e.id].y + nodeHeight(e.fields.length))) + 28;
    return [
      {
        id: `canvas-group:${group.id}`,
        name: group.name,
        groupId: group.id,
        color: /^#[0-9a-f]{6}$/i.test(group.color ?? '') ? group.color! : '#879b91',
        count: members.length,
        x,
        y,
        width: right - x,
        height: bottom - y,
      },
    ];
  });
}

/** Translate from a drag-start snapshot, including hidden members, without drift. */
export function translateGroup(
  positions: Record<string, Position>,
  memberIds: string[],
  delta: Position,
): Record<string, Position> {
  const result = { ...positions };
  for (const id of new Set(memberIds)) {
    if (positions[id]) result[id] = { x: positions[id].x + delta.x, y: positions[id].y + delta.y };
  }
  return result;
}

/** Saved layouts bypass Dagre. Unpositioned graphs reuse a layout until topology changes. */
export function createPositionResolver(solve = gridPositions) {
  let topology = '';
  let fallback: Record<string, Position> = {};
  return (graph: Graph, saved: Record<string, Position>): Record<string, Position> => {
    const valid = (id: string) => Number.isFinite(saved[id]?.x) && Number.isFinite(saved[id]?.y);
    if (graph.entities.every((entity) => valid(entity.id))) return saved;
    const key = JSON.stringify([
      graph.entities.map((entity) => [entity.id, entity.fields.length]),
      graph.relations.map((edge) => [edge.id, edge.source, edge.target]),
    ]);
    if (key !== topology) {
      fallback = solve(graph);
      topology = key;
    }
    return Object.fromEntries(
      graph.entities.map((entity) => [
        entity.id,
        valid(entity.id) ? saved[entity.id] : fallback[entity.id],
      ]),
    );
  };
}

/** Bounds use model positions, including nodes that have never mounted in the viewport. */
export function graphBounds(graph: Graph, positions: Record<string, Position>, ids?: Set<string>) {
  const entities = graph.entities.filter(
    (entity) =>
      (!ids || ids.has(entity.id)) &&
      Number.isFinite(positions[entity.id]?.x) &&
      Number.isFinite(positions[entity.id]?.y),
  );
  if (!entities.length) return null;
  const x = Math.min(...entities.map((entity) => positions[entity.id].x));
  const y = Math.min(...entities.map((entity) => positions[entity.id].y));
  return {
    x,
    y,
    width: Math.max(...entities.map((entity) => positions[entity.id].x + NODE_WIDTH)) - x,
    height:
      Math.max(
        ...entities.map((entity) => positions[entity.id].y + nodeHeight(entity.fields.length)),
      ) - y,
  };
}

/** Immediate fallback while the worker computes a relationship-aware arrangement. */
export function gridPositions(graph: Graph): Record<string, Position> {
  const columns = Math.max(1, Math.ceil(Math.sqrt(graph.entities.length)));
  const height = Math.max(160, ...graph.entities.map((e) => nodeHeight(e.fields.length))) + 60;
  return Object.fromEntries(
    graph.entities.map((e, i) => [
      e.id,
      { x: 40 + (i % columns) * (NODE_WIDTH + 70), y: 60 + Math.floor(i / columns) * height },
    ]),
  );
}
