import { type Event, listen, type UnlistenFn } from '@tauri-apps/api/event';

import { isReconciliationActive } from '$lib/api/config';
import { auditState } from '$lib/stores/audit.svelte';
import { filesState } from '$lib/stores/files.svelte';

const REFRESH_DEBOUNCE_MS = 50;

type RefreshTarget = {
  refresh: () => Promise<void>;
};

function createCoalescedRefresh(target: RefreshTarget) {
  let active = true;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let inFlight = false;
  let queuedWhileInFlight = false;

  function clearTimer() {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  }

  async function run() {
    timer = null;
    if (!active) return;

    if (inFlight) {
      queuedWhileInFlight = true;
      return;
    }

    inFlight = true;
    try {
      await target.refresh();
    } finally {
      inFlight = false;
      if (queuedWhileInFlight && active) {
        queuedWhileInFlight = false;
        request();
      }
    }
  }

  function request() {
    if (!active) return;
    if (inFlight) {
      queuedWhileInFlight = true;
      return;
    }
    if (timer) return;

    timer = setTimeout(() => {
      void run();
    }, REFRESH_DEBOUNCE_MS);
  }

  function stop() {
    active = false;
    clearTimer();
  }

  return { request, stop };
}

let stopActiveSnapshots: (() => void) | null = null;

export function startLiveSnapshots(): () => void {
  if (typeof window === 'undefined') return () => {};
  if (stopActiveSnapshots) return () => {};

  let stopped = false;
  const unlistenTasks: Promise<UnlistenFn>[] = [];
  const filesRefresh = createCoalescedRefresh(filesState);
  const auditRefresh = createCoalescedRefresh(auditState);

  function register<T>(eventName: string, handler: (event: Event<T>) => void) {
    const task = listen<T>(eventName, handler).catch((reason) => {
      console.error(`Failed to listen for ${eventName}.`, reason);
      return () => {};
    });
    unlistenTasks.push(task);
  }

  register('reconciliation_started', () => {
    filesState.beginSync();
  });

  register<[string, number, number]>('reconciliation_progress', ({ payload }) => {
    const [path, current, total] = payload;
    filesState.updateSyncProgress(path, current, total);
  });

  register('reconciliation_completed', () => {
    filesState.completeSync();
    filesRefresh.request();
  });

  register('action_completed', () => {
    filesRefresh.request();
  });

  register('audit_updated', () => {
    auditRefresh.request();
  });

  const refreshOnPathEvent = () => {
    if (!filesState.syncing) filesRefresh.request();
  };

  register('file_indexed', refreshOnPathEvent);
  register('file_updated', refreshOnPathEvent);
  register('file_removed', refreshOnPathEvent);

  const refreshFilesOnFocus = () => {
    filesRefresh.request();
  };
  window.addEventListener('focus', refreshFilesOnFocus);

  void isReconciliationActive()
    .then((active) => {
      if (!stopped && active) {
        filesState.beginSync();
      }
    })
    .catch((reason) => {
      console.error('Failed to read reconciliation state.', reason);
    });

  filesRefresh.request();
  auditRefresh.request();

  const stop = () => {
    if (stopped) return;
    stopped = true;
    stopActiveSnapshots = null;
    window.removeEventListener('focus', refreshFilesOnFocus);
    filesRefresh.stop();
    auditRefresh.stop();
    filesState.completeSync();

    for (const task of unlistenTasks) {
      void task.then((unlisten) => unlisten());
    }
  };

  stopActiveSnapshots = stop;
  return stop;
}
