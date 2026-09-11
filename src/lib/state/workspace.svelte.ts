import { api } from '$lib/services/workspace';
import type { Project, Source } from '$lib/types';
export class WorkspaceState {
  projects = $state.raw<Project[]>([]);
  projectId = $state<string | null>(null);
  sourceId = $state<string | null>(null);
  loading = $state(true);
  error = $state('');
  notice = $state('');
  project = $derived(this.projects.find((p) => p.id === this.projectId) ?? null);
  source = $derived(this.project?.sources.find((s) => s.id === this.sourceId) ?? null);
  async load() {
    try {
      this.projects = await api.list();
      const saved = localStorage.getItem('atlas:last-project');
      this.selectProject(
        this.projects.find((p) => p.id === saved)?.id ?? this.projects[0]?.id ?? null,
      );
    } catch (e) {
      this.fail(e);
    } finally {
      this.loading = false;
    }
  }
  selectProject(id: string | null) {
    this.projectId = id;
    this.sourceId = this.projects.find((p) => p.id === id)?.sources[0]?.id ?? null;
    if (id) localStorage.setItem('atlas:last-project', id);
  }
  selectSource(source: Source) {
    this.sourceId = source.id;
  }
  /** Swap in a newer copy of a known project without changing the selection. */
  replace(project: Project) {
    this.projects = this.projects.map((p) => (p.id === project.id ? project : p));
  }
  upsert(project: Project, selectLast = false) {
    if (this.projects.some((p) => p.id === project.id)) this.replace(project);
    else this.projects = [project, ...this.projects];
    if (this.projectId !== project.id) this.selectProject(project.id);
    if (selectLast) this.sourceId = project.sources.at(-1)?.id ?? null;
    if (!project.sources.some((s) => s.id === this.sourceId))
      this.sourceId = project.sources[0]?.id ?? null;
  }
  fail(error: unknown) {
    this.error = error instanceof Error ? error.message : String(error);
  }
  async deleteProject() {
    if (!this.project) return;
    await api.delete(this.project.id);
    this.projects = this.projects.filter((p) => p.id !== this.projectId);
    this.selectProject(this.projects[0]?.id ?? null);
  }
}
