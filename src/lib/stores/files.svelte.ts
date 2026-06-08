import { getActiveFiles } from '$lib/api/files';
import type { TrackedFile } from '$lib/types';
import { getErrorMessage } from '$lib/utils/format';

class FilesState {
  files = $state<TrackedFile[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);

  private hasLoadedOnce = false;
  private loadingTimeout: ReturnType<typeof setTimeout> | null = null;

  counts = $derived.by(() => {
    let fresh = 0;
    let stale = 0;
    let decaying = 0;
    let pinned = 0;
    let ignored = 0;

    for (let i = 0; i < this.files.length; i++) {
      const state = this.files[i].state;
      if (state === 'Fresh') fresh++;
      else if (state === 'Stale') stale++;
      else if (state === 'Decaying') decaying++;
      else if (state === 'Pinned') pinned++;
      else if (state === 'Ignored') ignored++;
    }

    return { fresh, stale, decaying, pinned, ignored };
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
      this.error = getErrorMessage(error, 'Could not load files.');
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
