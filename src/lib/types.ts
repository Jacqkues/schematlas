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
  id: string;
  name: string;
  description: string;
  createdAt: string;
  updatedAt: string;
  sources: Source[];
}
export interface ConnectionRequest {
  kind: DatabaseKind;
  connectionString: string;
}
export const databaseNames: Record<DatabaseKind, string> = {
  postgres: 'PostgreSQL',
  mysql: 'MySQL',
  mariadb: 'MariaDB',
  sqlite: 'SQLite',
  mssql: 'SQL Server',
};
