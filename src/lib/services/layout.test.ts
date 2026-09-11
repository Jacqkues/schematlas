import { describe, it, expect, vi } from 'vitest';
import {
  groupBounds,
  graphBounds,
  createPositionResolver,
  translateGroup,
  layoutGraph,
  matchingIds,
  filterGraph,
  schemaNamespaces,
  NODE_WIDTH,
  nodeHeight,
} from './layout';
import sample from './sample.json';
import type { Graph } from '$lib/types';
const graph = sample.sources[0].graph as Graph;
describe('graph layout', () => {
  it('positions every node finitely with no overlapping bounds', () => {
    const positions = layoutGraph(graph);
    expect(Object.keys(positions)).toHaveLength(graph.entities.length);
    for (const a of graph.entities) {
      const p = positions[a.id];
      expect(Number.isFinite(p.x) && Number.isFinite(p.y)).toBe(true);
      for (const b of graph.entities) {
        if (a.id === b.id) continue;
        const q = positions[b.id];
        expect(
          p.x + NODE_WIDTH <= q.x ||
            q.x + NODE_WIDTH <= p.x ||
            p.y + nodeHeight(a.fields.length) <= q.y ||
            q.y + nodeHeight(b.fields.length) <= p.y,
        ).toBe(true);
      }
    }
  });
  it('finds columns case-insensitively and handles empty graphs', () => {
    expect(matchingIds(graph, 'CUSTOMER_ID').size).toBe(2);
    expect(matchingIds(graph, 'nonsense').size).toBe(0);
    expect(layoutGraph({ entities: [], relations: [], warnings: [] })).toEqual({});
  });
});

describe('schema filtering', () => {
  const multi: Graph = {
    entities: ['sales', 'billing', 'audit'].map((namespace) => ({
      ...graph.entities[0],
      id: `${namespace}:users`,
      namespace,
      name: 'users',
    })),
    relations: [
      { ...graph.relations[0], id: 'cross-schema', source: 'sales:users', target: 'billing:users' },
    ],
    warnings: [],
  };
  it('keeps identical table names distinct and counts every schema', () => {
    expect(schemaNamespaces(multi)).toEqual([
      { name: 'audit', count: 1 },
      { name: 'billing', count: 1 },
      { name: 'sales', count: 1 },
    ]);
    expect(filterGraph(multi, ['sales'], false).entities.map((e) => e.id)).toEqual(['sales:users']);
    expect(filterGraph(multi, ['sales'], false).relations).toHaveLength(0);
    expect(filterGraph(multi, [], false)).toBe(multi);
  });
  it('includes directly linked schemas only when requested', () => {
    const filtered = filterGraph(multi, ['sales'], true);
    expect(filtered.entities.map((e) => e.namespace)).toEqual(['sales', 'billing']);
    expect(filtered.relations).toHaveLength(1);
    expect(filterGraph(multi, ['sales', 'audit'], false).entities).toHaveLength(2);
  });
});

describe('named group overlays', () => {
  it('encloses member nodes and follows moves without changing coordinates', () => {
    const members = graph.entities.slice(0, 2);
    const positions = { [members[0].id]: { x: 100, y: 200 }, [members[1].id]: { x: 500, y: 200 } };
    const groups = [{ id: 'domain', name: 'Customer domain', nodeIds: members.map((e) => e.id) }];
    const initial = groupBounds(groups, graph, positions)[0];
    expect(initial.x).toBe(72);
    expect(initial.y).toBe(152);
    expect(initial.width).toBe(740);
    expect(initial.count).toBe(2);
    positions[members[1].id] = { x: 900, y: 500 };
    const moved = groupBounds(groups, graph, positions)[0];
    expect(moved.width).toBe(1140);
    expect(moved.height).toBeGreaterThan(initial.height);
    expect(groupBounds(groups, { ...graph, entities: [] }, positions)).toEqual([]);
  });
});

describe('group dragging', () => {
  it('moves every member from the initial snapshot, leaving unrelated nodes unchanged', () => {
    const positions = {
      visible: { x: 10, y: 20 },
      hidden: { x: 300, y: 80 },
      other: { x: 900, y: 400 },
    };
    const members = ['visible', 'hidden', 'hidden', 'missing'];
    const moved = translateGroup(positions, members, { x: 80, y: -50 });
    expect(moved).toEqual({
      visible: { x: 90, y: -30 },
      hidden: { x: 380, y: 30 },
      other: positions.other,
    });
    expect(positions.visible).toEqual({ x: 10, y: 20 });
    expect(translateGroup(positions, members, { x: 100, y: 0 }).hidden).toEqual({ x: 400, y: 80 });
  });
});

describe('saved layout performance', () => {
  it('does not run Dagre for a fully positioned graph', () => {
    const solve = vi.fn(layoutGraph);
    const resolve = createPositionResolver(solve);
    const saved = layoutGraph(graph);
    expect(resolve(graph, saved)).toBe(saved);
    expect(resolve(structuredClone(graph), saved)).toBe(saved);
    expect(solve).not.toHaveBeenCalled();
  });
  it('reuses the fallback across snapshot copies but recalculates changed topology', () => {
    const solve = vi.fn(layoutGraph);
    const resolve = createPositionResolver(solve);
    resolve(graph, {});
    resolve(structuredClone(graph), {});
    expect(solve).toHaveBeenCalledTimes(1);
    const changed = structuredClone(graph);
    changed.entities[0].fields.push({ ...changed.entities[0].fields[0], name: 'extra' });
    const position = { x: 45, y: 90 };
    expect(resolve(changed, { [changed.entities[0].id]: position })[changed.entities[0].id]).toBe(
      position,
    );
    expect(solve).toHaveBeenCalledTimes(2);
  });
});

describe('viewport bounds', () => {
  it('includes distant, unmounted nodes and respects the requested focus', () => {
    const distant: Graph = { ...graph, entities: graph.entities.slice(0, 2), relations: [] };
    const [a, b] = distant.entities;
    const positions = { [a.id]: { x: -9000, y: -6000 }, [b.id]: { x: 24000, y: 19000 } };
    expect(graphBounds(distant, positions)).toEqual({
      x: -9000,
      y: -6000,
      width: 33000 + NODE_WIDTH,
      height: 25000 + nodeHeight(b.fields.length),
    });
    expect(graphBounds(distant, positions, new Set([b.id]))).toEqual({
      x: 24000,
      y: 19000,
      width: NODE_WIDTH,
      height: nodeHeight(b.fields.length),
    });
    expect(graphBounds(distant, positions, new Set())).toBeNull();
  });
});
