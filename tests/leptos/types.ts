// Wire-format types shared by the browser regression tests. They mirror the
// serde representation of src-tauri/src/domain.rs, which the Leptos frontend
// receives from the Tauri commands.
export type DatabaseKind = 'postgres' | 'mysql' | 'mariadb' | 'sqlite' | 'mssql';
export interface Field {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
  required: boolean;
  defaultValue: string | null;
  description: string | null;
}
export interface Entity {
  id: string;
  name: string;
  namespace: string;
  kind: 'table' | 'view' | 'schema' | 'operation';
  description: string | null;
  fields: Field[];
  method?: string;
  uniqueKeys?: string[][] | null;
}
export interface Relation {
  id: string;
  source: string;
  target: string;
  sourceField: string | null;
  targetField: string | null;
  label: string;
}
export interface Graph {
  entities: Entity[];
  relations: Relation[];
  warnings: string[];
}
export interface Position {
  x: number;
  y: number;
}
export interface CanvasGroup {
  id: string;
  name: string;
  nodeIds: string[];
  color?: string;
}
export interface Source {
  groups?: CanvasGroup[];
  layoutBackup?: { positions: Record<string, Position>; groups: CanvasGroup[] } | null;
  apiBaseUrl?: string | null;
  id: string;
  name: string;
  kind: 'database' | 'openapi';
  databaseKind: DatabaseKind | null;
  importedAt: string;
  graph: Graph;
  positions: Record<string, Position>;
}
export interface Project {
  workingDirectory?: string | null;
  agentSessionId?: string | null;
  id: string;
  name: string;
  description: string;
  createdAt: string;
  updatedAt: string;
  sources: Source[];
}
