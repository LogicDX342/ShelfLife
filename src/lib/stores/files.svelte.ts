import { getActiveFiles } from '$lib/api/files';
import type { TrackedFile } from '$lib/types';

class FilesState {
  files = $state<TrackedFile[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);

  private hasLoadedOnce = false;
  private loadingTimeout: ReturnType<typeof setTimeout> | null = null;

  counts = $derived({
    fresh: this.files.filter((file) => file.state === 'Fresh').length,
    stale: this.files.filter((file) => file.state === 'Stale').length,
    decaying: this.files.filter((file) => file.state === 'Decaying').length,
    pinned: this.files.filter((file) => file.state === 'Pinned').length,
    ignored: this.files.filter((file) => file.state === 'Ignored').length,
  });

  async refresh() {
    this.error = null;

    if (this.loadingTimeout) {
      clearTimeout(this.loadingTimeout);
      this.loadingTimeout = null;
    }

    if (!this.hasLoadedOnce) {
      this.loading = true;
    } else {
      this.loadingTimeout = setTimeout(() => {
        this.loading = true;
      }, 500);
    }

    try {
      this.files = await getActiveFiles();
      this.hasLoadedOnce = true;
    } catch (error) {
      this.error = error instanceof Error ? error.message : 'Could not load files.';
    } finally {
      if (this.loadingTimeout) {
        clearTimeout(this.loadingTimeout);
        this.loadingTimeout = null;
      }
      this.loading = false;
    }
  }
}

export const filesState = new FilesState();
