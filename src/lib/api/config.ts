import { invoke } from '@tauri-apps/api/core';

import type { AppConfig, CloseBehavior, WatchTarget } from '$lib/types';

export function getConfig() {
  return invoke<AppConfig>('get_config');
}

export function saveConfig(config: AppConfig) {
  return invoke<AppConfig>('save_config', { config });
}

export function resolveCloseRequest(behavior: CloseBehavior, remember: boolean) {
  return invoke<void>('resolve_close_request', { behavior, remember });
}

export function updateWatchTargets(targets: WatchTarget[]) {
  return invoke<void>('update_watch_targets', { targets });
}

export function runReconciliationScan() {
  return invoke<void>('run_reconciliation_scan');
}

export function isReconciliationActive() {
  return invoke<boolean>('is_reconciliation_active');
}

export function pauseWatching() {
  return invoke<void>('pause_watching');
}

export function resumeWatching() {
  return invoke<void>('resume_watching');
}
