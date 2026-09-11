/** Browser-only design preview. Database and import operations require Rust IPC. */
import type { Project, Position } from '$lib/types';
import sample from './sample.json';
const key = 'schema-atlas-browser-preview-v1';
function load(): Project[] {
  return JSON.parse(localStorage.getItem(key) ?? '[]') as Project[];
}
function save(projects: Project[]) {
  localStorage.setItem(key, JSON.stringify(projects));
}
export async function preview<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const projects = load();
  let result: unknown;
  const project = projects.find((p) => p.id === args.projectId);
  switch (command) {
    case 'undo_canvas':
    case 'remove_canvas_group': {
      const source = project?.sources.find((s) => s.id === args.sourceId);
      if (!source || !project) throw new Error('Source not found.');
      if (command === 'undo_canvas') {
        if (!source.layoutBackup) throw new Error('No canvas edit to undo.');
        source.positions = source.layoutBackup.positions;
        source.groups = source.layoutBackup.groups;
        source.layoutBackup = null;
      } else {
        source.layoutBackup = {
          positions: structuredClone(source.positions),
          groups: structuredClone(source.groups ?? []),
        };
        source.groups = source.groups?.filter((g) => g.id !== args.groupId);
      }
      result = project;
      break;
    }

    case 'set_canvas_group': {
      const input = args.args as {
        source_id: string;
        group_id: string | null;
        name: string;
        node_ids: string[];
        color: string | null;
      };
      const source = project?.sources.find((s) => s.id === input.source_id);
      if (!source || !project) throw new Error('Source not found.');
      const previous = source.groups?.find((g) => g.id === input.group_id);
      if (input.group_id && !previous) throw new Error('Group not found.');
      const color = input.color ?? previous?.color ?? '#879b91';
      if (
        !input.name.trim() ||
        input.name.trim().length > 80 ||
        !/^#[0-9a-f]{6}$/i.test(color) ||
        !input.node_ids.length ||
        new Set(input.node_ids).size !== input.node_ids.length ||
        input.node_ids.some((id) => !source.graph.entities.some((e) => e.id === id))
      )
        throw new Error('Choose a name, color, and valid group members.');
      if (!previous && (source.groups?.length ?? 0) >= 100)
        throw new Error('Use at most 100 groups per source.');
      source.layoutBackup = {
        positions: structuredClone(source.positions),
        groups: structuredClone(source.groups ?? []),
      };
      const group = {
        id: previous?.id ?? crypto.randomUUID(),
        name: input.name.trim(),
        nodeIds: input.node_ids,
        color: color.toLowerCase(),
      };
      source.groups = previous
        ? source.groups!.map((g) => (g.id === group.id ? group : g))
        : [...(source.groups ?? []), group];
      result = project;
      break;
    }
    case 'list_projects':
      return projects as T;
    case 'create_demo': {
      const demo = structuredClone(sample) as Project;
      demo.id = crypto.randomUUID();
      demo.createdAt = demo.updatedAt = new Date().toISOString();
      for (const source of demo.sources) source.id = crypto.randomUUID();
      projects.push(demo);
      result = demo;
      break;
    }
    case 'create_project': {
      const p: Project = {
        id: crypto.randomUUID(),
        name: String(args.name).trim(),
        description: String(args.description),
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        sources: [],
      };
      projects.push(p);
      result = p;
      break;
    }
    case 'rename_project':
      if (project) {
        project.name = String(args.name);
        project.description = String(args.description);
        result = project;
      }
      break;
    case 'delete_project':
      save(projects.filter((p) => p.id !== args.projectId));
      return undefined as T;
    case 'remove_source':
      if (project) {
        project.sources = project.sources.filter((s) => s.id !== args.sourceId);
        result = project;
      }
      break;
    case 'save_positions': {
      const source = project?.sources.find((s) => s.id === args.sourceId);
      if (!source || !project) throw new Error('Source not found.');
      source.layoutBackup = {
        positions: structuredClone(source.positions),
        groups: structuredClone(source.groups ?? []),
      };
      source.positions = { ...source.positions, ...(args.positions as Record<string, Position>) };
      result = project;
      break;
    }
    default:
      throw new Error(
        'Open the Schematlas desktop app to connect databases or import OpenAPI files.',
      );
  }
  save(projects);
  return result as T;
}
