import { invoke } from '@tauri-apps/api/core';
import type { AuditEntry, BulkTriageResult, UserTriageAction } from '$lib/types';

export function executeTriageAction(path: string, action: UserTriageAction) {
  return invoke<AuditEntry>('execute_triage_action', { path, action });
}

export function executeBulkTriageAction(paths: string[], action: UserTriageAction) {
  return invoke<BulkTriageResult>('execute_bulk_triage_action', { paths, action });
}

export function undoAuditEntry(auditId: string) {
  return invoke<AuditEntry>('undo_audit_entry', { auditId });
}

export function listAuditEntries() {
  return invoke<AuditEntry[]>('list_audit_entries');
}
