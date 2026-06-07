import { listAuditEntries } from '$lib/api/triage';
import type { AuditEntry } from '$lib/types';

class AuditState {
  entries = $state<AuditEntry[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);

  async refresh() {
    this.loading = true;
    this.error = null;
    try {
      this.entries = await listAuditEntries();
    } catch (error) {
      this.error = error instanceof Error ? error.message : 'Could not load audit entries.';
    } finally {
      this.loading = false;
    }
  }
}

export const auditState = new AuditState();
