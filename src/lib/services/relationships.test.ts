import { describe, expect, it } from 'vitest';
import { relationships, neighborhood } from './relationships';
import type { Entity, Graph } from '$lib/types';
const table = (id: string, keys?: string[][]): Entity => ({
  id,
  name: id,
  namespace: 'main',
  kind: 'table',
  description: null,
  uniqueKeys: keys,
  fields: ['a', 'b'].map((name) => ({
    name,
    dataType: 'int',
    nullable: false,
    primaryKey: id === 'parent',
    required: true,
    defaultValue: null,
    description: null,
  })),
});
function graph(keys?: string[][]): Graph {
  return {
    entities: [table('parent'), table('child', keys)],
    warnings: [],
    relations: ['a', 'b'].map((name) => ({
      id: name,
      source: 'child',
      target: 'parent',
      sourceField: name,
      targetField: name,
      label: 'fk_pair',
    })),
  };
}
describe('relationship cardinality', () => {
  it('coalesces composite constraints and recognizes composite uniqueness', () => {
    const result = relationships(graph([['a', 'b']]), true);
    expect(result).toHaveLength(1);
    expect(result[0].sourceCardinality).toBe('0..1');
    expect(result[0].targetCardinality).toBe('1');
  });
  it('distinguishes nullable many-to-one from one-to-one', () => {
    const g = graph([]);
    g.entities[1].fields[1].nullable = true;
    const r = relationships(g, true)[0];
    expect(r.sourceCardinality).toBe('0..N');
    expect(r.targetCardinality).toBe('0..1');
  });
  it('does not invent uniqueness for older snapshots or OpenAPI refs', () => {
    expect(relationships(graph(), true)[0].sourceCardinality).toBe('0..?');
    expect(relationships(graph(), false)).toHaveLength(2);
    expect(relationships(graph(), false)[0].sourceCardinality).toBe('');
  });
  it('includes incoming, outgoing, and self relations, but not unrelated nodes', () => {
    const g = graph();
    g.entities.push(table('unrelated'));
    expect([...neighborhood(g, 'parent')].sort()).toEqual(['child', 'parent']);
    expect(neighborhood(g, null).size).toBe(0);
  });
});
