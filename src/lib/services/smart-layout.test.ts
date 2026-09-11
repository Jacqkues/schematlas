import { expect, it } from 'vitest';
import { smartLayout } from './smart-layout';
import { graphBounds, nodeHeight, NODE_WIDTH, groupBounds } from './layout';
import type { Graph } from '$lib/types';
it('packs a large cyclic, disconnected, grouped schema without overlapping tables or domains', () => {
  const graph: Graph = {
    entities: Array.from({ length: 164 }, (_, i) => ({
      id: `n${i}`,
      name: `table${i}`,
      namespace: i < 140 ? 'public' : 'auth',
      kind: 'table',
      description: null,
      fields: Array.from({ length: i % 12 }, (_, j) => ({
        name: `f${j}`,
        dataType: 'int',
        nullable: false,
        primaryKey: false,
        required: true,
        defaultValue: null,
        description: null,
      })),
    })),
    relations: [],
    warnings: [],
  };
  for (let i = 0; i < 150; i++)
    graph.relations.push({
      id: `r${i}`,
      source: `n${i}`,
      target: `n${Math.floor(i / 10) * 10 + ((i + 1) % 10)}`,
      sourceField: null,
      targetField: null,
      label: 'fk',
    });
  const groups = Array.from({ length: 16 }, (_, i) => ({
    id: `g${i}`,
    name: `Domain ${i}`,
    nodeIds: graph.entities.slice(i * 10, i * 10 + 10).map((e) => e.id),
  }));
  const positions = smartLayout(graph, groups);
  expect(Object.keys(positions)).toHaveLength(164);
  for (let i = 0; i < graph.entities.length; i++)
    for (let j = i + 1; j < graph.entities.length; j++) {
      const a = graph.entities[i],
        b = graph.entities[j],
        p = positions[a.id],
        q = positions[b.id];
      expect(
        p.x + NODE_WIDTH <= q.x ||
          q.x + NODE_WIDTH <= p.x ||
          p.y + nodeHeight(a.fields.length) <= q.y ||
          q.y + nodeHeight(b.fields.length) <= p.y,
      ).toBe(true);
    }
  const bounds = groupBounds(groups, graph, positions);
  for (let i = 0; i < bounds.length; i++)
    for (let j = i + 1; j < bounds.length; j++) {
      const a = bounds[i],
        b = bounds[j];
      expect(
        a.x + a.width <= b.x ||
          b.x + b.width <= a.x ||
          a.y + a.height <= b.y ||
          b.y + b.height <= a.y,
      ).toBe(true);
    }
  const area = graphBounds(graph, positions)!;
  expect(area.width / area.height).toBeGreaterThan(0.5);
  expect(area.width / area.height).toBeLessThan(3);
  expect(smartLayout(graph, groups)).toEqual(positions);
});
it('handles overlapping groups, singleton graphs, and empty graphs', () => {
  expect(smartLayout({ entities: [], relations: [], warnings: [] }, [])).toEqual({});
  const graph: Graph = {
    entities: ['a', 'b', 'c'].map((id) => ({
      id,
      name: id,
      namespace: 'public',
      kind: 'table',
      description: null,
      fields: [],
    })),
    relations: [],
    warnings: [],
  };
  const p = smartLayout(graph, [
    { id: 'g1', name: 'One', nodeIds: ['a', 'b'] },
    { id: 'g2', name: 'Two', nodeIds: ['b', 'c', 'missing'] },
  ]);
  expect(Object.keys(p).sort()).toEqual(['a', 'b', 'c']);
});
