import { invoke } from '@tauri-apps/api/core';
export interface Review {
  id: string;
  title: string;
  kind: string;
  details: unknown;
  options: { optionId: string; name: string; kind: string }[];
}
export interface AgentSnapshot {
  projectId: string;
  status: string;
  agentName: string;
  sessionId: string | null;
  messages: { id: string; role: string; text: string; status: string | null }[];
  reviews: Review[];
  authMethods: { id: string; name: string; description?: string }[];
  error: string | null;
  activity: string;
  lastActivityAt: number;
  turnStartedAt: number | null;
}
export const agent = {
  workingDirectory: (projectId: string) => invoke<string>('agent_working_directory', { projectId }),
  status: (projectId: string) => invoke<AgentSnapshot | null>('agent_status', { projectId }),
  connect: (projectId: string, config: { executable: string; args: string[]; cwd: string }) =>
    invoke('agent_connect', { projectId, config }),
  prompt: (projectId: string, text: string) => invoke('agent_prompt', { projectId, text }),
  cancel: (projectId: string) => invoke('agent_cancel', { projectId }),
  disconnect: (projectId: string) => invoke('agent_disconnect', { projectId }),
  decide: (projectId: string, reviewId: string, option: string | null) =>
    invoke('agent_decide', { projectId, reviewId, option }),
  authenticate: (projectId: string, methodId: string) =>
    invoke('agent_authenticate', { projectId, methodId }),
};
