import { invoke } from '@tauri-apps/api/core';

export function openDiagnosticLogs() {
  return invoke<void>('open_diagnostic_logs');
}
