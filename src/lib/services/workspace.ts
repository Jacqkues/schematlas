import { invoke, isTauri } from '@tauri-apps/api/core';
import type { CanvasGroup, ConnectionRequest, Position, Project } from '$lib/types';
import { preview } from './preview';
export const desktop = isTauri();
const call = <T>(command: string, args: Record<string, unknown> = {}): Promise<T> =>
  desktop ? invoke<T>(command, args) : preview<T>(command, args);
export const api = {
  setGroup: (
    projectId: string,
    sourceId: string,
    group: Omit<CanvasGroup, 'id'> & { id?: string },
  ) =>
    call<Project>('set_canvas_group', {
      projectId,
      args: {
        source_id: sourceId,
        group_id: group.id ?? null,
        name: group.name,
        node_ids: group.nodeIds,
        color: group.color ?? null,
      },
    }),
  undoCanvas: (projectId: string, sourceId: string) =>
    call<Project>('undo_canvas', { projectId, sourceId }),
  removeGroup: (projectId: string, sourceId: string, groupId: string) =>
    call<Project>('remove_canvas_group', { projectId, sourceId, groupId }),
  list: () => call<Project[]>('list_projects'),
  create: (name: string, description: string) =>
    call<Project>('create_project', { name, description }),
  rename: (projectId: string, name: string, description: string) =>
    call<Project>('rename_project', { projectId, name, description }),
  delete: (projectId: string) => call<void>('delete_project', { projectId }),
  demo: () => call<Project>('create_demo'),
  connect: (projectId: string, name: string, request: ConnectionRequest, sourceId?: string) =>
    call<Project>('connect_database', { projectId, name, request, sourceId: sourceId ?? null }),
  refresh: (projectId: string, sourceId: string) =>
    call<Project>('refresh_source', { projectId, sourceId }),
  import: (projectId: string, document: string) =>
    call<Project>('import_openapi', { projectId, document }),
  removeSource: (projectId: string, sourceId: string) =>
    call<Project>('remove_source', { projectId, sourceId }),
  savePositions: (projectId: string, sourceId: string, positions: Record<string, Position>) =>
    call<Project>('save_positions', { projectId, sourceId, positions }),
  export: (projectId: string, sourceId: string, path: string) =>
    call<void>('export_source', { projectId, sourceId, path }),
};
