import { listAuditEntries } from '$lib/api/triage';
import type { AuditEntry } from '$lib/types';
import { getErrorMessage } from '$lib/utils/format';

class AuditState {
  entries = $state<AuditEntry[]>([]);
  loading = $state(false);
  loadingMore = $state(false);
  error = $state<string | null>(null);
  hasMore = $state(false);
  totalCount = $state(0);
  searchQuery = $state('');

  private requestVersion = 0;

  async setSearchQuery(query: string) {
    if (query === this.searchQuery) return;

    this.searchQuery = query;
    this.entries = [];
    this.hasMore = false;
    this.totalCount = 0;
    await this.refresh();
  }

  async refresh() {
    const requestVersion = ++this.requestVersion;
    this.loading = true;
    this.loadingMore = false;
    this.error = null;
    try {
      const page = await listAuditEntries(null, this.searchQuery);
      if (requestVersion !== this.requestVersion) return;

      this.entries = page.entries;
      this.hasMore = page.has_more;
      this.totalCount = page.total_count ?? page.entries.length;
    } catch (error) {
      if (requestVersion !== this.requestVersion) return;
      this.error = getErrorMessage(error, 'Could not load audit entries.');
    } finally {
      if (requestVersion === this.requestVersion) {
        this.loading = false;
      }
    }
  }

  async loadMore() {
    if (this.loading || this.loadingMore || !this.hasMore) return;

    const cursor = this.entries.at(-1)?.sequence;
    if (cursor === undefined) return;

    const requestVersion = this.requestVersion;
    const searchQuery = this.searchQuery;
    this.loadingMore = true;
    this.error = null;
    try {
      const page = await listAuditEntries(cursor, searchQuery);
      if (requestVersion !== this.requestVersion || searchQuery !== this.searchQuery) return;

      this.entries = [...this.entries, ...page.entries];
      this.hasMore = page.has_more;
    } catch (error) {
      if (requestVersion !== this.requestVersion) return;
      this.error = getErrorMessage(error, 'Could not load more audit entries.');
    } finally {
      if (requestVersion === this.requestVersion) {
        this.loadingMore = false;
      }
    }
  }
}

export const auditState = new AuditState();
