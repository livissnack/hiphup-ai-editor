import { invoke } from '@tauri-apps/api/core';
import { ref } from 'vue';

export type AgentStep = {
  index: number;
  title: string;
  status: string;
  detail: string;
};

export type AgentRun = {
  id: string;
  goal: string;
  status: string;
  steps: AgentStep[];
  summary?: string | null;
};

export const useAgentRun = () => {
  const run = ref<AgentRun | null>(null);
  const loading = ref(false);
  const error = ref('');
  let pollTimer: number | null = null;

  const stopPolling = () => {
    if (pollTimer !== null) {
      window.clearInterval(pollTimer);
      pollTimer = null;
    }
  };

  const ensurePolling = () => {
    if (pollTimer !== null) return;
    pollTimer = window.setInterval(async () => {
      if (!run.value?.id) return;
      if (run.value.status !== 'running') {
        stopPolling();
        return;
      }
      await refresh();
    }, 800);
  };

  const start = async (goal: string) => {
    const text = goal.trim();
    if (!text) return;
    loading.value = true;
    error.value = '';
    try {
      run.value = await invoke<AgentRun>('agent_start_run', { payload: { goal: text } });
      if (run.value?.status === 'running') ensurePolling();
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  };

  const refresh = async () => {
    if (!run.value?.id) return;
    try {
      const latest = await invoke<AgentRun | null>('agent_get_run', { runId: run.value.id });
      if (latest) {
        run.value = latest;
        if (latest.status !== 'running') stopPolling();
      }
    } catch (e) {
      error.value = String(e);
    }
  };

  const stop = async () => {
    if (!run.value?.id) return;
    try {
      await invoke<boolean>('agent_stop_run', { runId: run.value.id });
      await refresh();
      stopPolling();
    } catch (e) {
      error.value = String(e);
    }
  };

  return {
    run,
    loading,
    error,
    start,
    refresh,
    stop,
  };
};
