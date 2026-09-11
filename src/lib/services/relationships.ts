import type { Entity, Graph, Relation } from '$lib/types';
export interface Relationship extends Relation {
  sourceCardinality: string;
  targetCardinality: string;
  description: string;
}
function isUnique(entity: Entity | undefined, columns: string[]): boolean | null {
  if (!entity || !columns.length) return null;
  const pk = entity.fields.filter((f) => f.primaryKey).map((f) => f.name);
  const keys = [...(entity.uniqueKeys ?? []), ...(pk.length ? [pk] : [])];
  if (keys.some((key) => key.length && key.every((name) => columns.includes(name)))) return true;
  return entity.uniqueKeys == null ? null : false;
}
/** A composite foreign key is one relationship, not one relationship per column. */
export function relationships(graph: Graph, database: boolean): Relationship[] {
  const entities = new Map(graph.entities.map((e) => [e.id, e]));
  const constraints = new Map<string, Relation[]>();
  for (const r of graph.relations) {
    const key = database ? JSON.stringify([r.source, r.target, r.label]) : r.id;
    constraints.set(key, [...(constraints.get(key) ?? []), r]);
  }
  return [...constraints.values()].map((parts) => {
    const first = parts[0];
    const from = entities.get(first.source),
      to = entities.get(first.target);
    const sourceColumns = parts.flatMap((r) => (r.sourceField ? [r.sourceField] : []));
    const targetColumns = parts.flatMap((r) => (r.targetField ? [r.targetField] : []));
    const sourceUnique = isUnique(from, sourceColumns),
      targetUnique = isUnique(to, targetColumns);
    const optional = sourceColumns.some(
      (name) => from?.fields.find((f) => f.name === name)?.nullable !== false,
    );
    const sourceCardinality = !database
      ? ''
      : sourceUnique === true
        ? '0..1'
        : sourceUnique === false
          ? '0..N'
          : '0..?';
    const targetCardinality = !database
      ? ''
      : targetUnique === true
        ? optional
          ? '0..1'
          : '1'
        : targetUnique === false
          ? optional
            ? '0..N'
            : '1..N'
          : optional
            ? '0..?'
            : '1..?';
    return {
      ...first,
      sourceField: parts.length === 1 ? first.sourceField : null,
      targetField: parts.length === 1 ? first.targetField : null,
      sourceCardinality,
      targetCardinality,
      description: database
        ? `${from?.name} (${sourceColumns.join(', ')}) → ${to?.name} (${targetColumns.join(', ')}). ${sourceCardinality} referencing rows per referenced row; ${targetCardinality} referenced rows per referencing row.${sourceUnique === null || targetUnique === null ? ' Refresh the schema to inspect unique keys; ? means unknown maximum.' : ''}`
        : first.label,
    };
  });
}
export function neighborhood(graph: Graph, selected: string | null): Set<string> {
  const ids = new Set<string>();
  if (!selected) return ids;
  ids.add(selected);
  for (const r of graph.relations)
    if (r.source === selected || r.target === selected) {
      ids.add(r.source);
      ids.add(r.target);
    }
  return ids;
}
