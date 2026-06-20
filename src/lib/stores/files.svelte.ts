import { getActiveFiles } from '$lib/api/files';
import type { TrackedFile } from '$lib/types';
import { getErrorMessage } from '$lib/utils/format';

class FilesState {
  files = $state<TrackedFile[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  syncing = $state(false);
  filesScanned = $state(0);
  filesTotal = $state(0);
  currentPath = $state('');
  syncDuration = $state(0);

  private hasLoadedOnce = false;
  private loadingTimeout: ReturnType<typeof setTimeout> | null = null;
  private timerInterval: ReturnType<typeof setInterval> | null = null;
  private syncStartTime = 0;

  private startTimer() {
    this.stopTimer();
    this.syncStartTime = Date.now();
    this.syncDuration = 0;
    this.timerInterval = setInterval(() => {
      this.syncDuration = (Date.now() - this.syncStartTime) / 1000;
    }, 100);
  }

  private stopTimer() {
    if (this.timerInterval) {
      clearInterval(this.timerInterval);
      this.timerInterval = null;
    }
  }

  beginSync() {
    this.syncing = true;
    this.filesScanned = 0;
    this.filesTotal = 0;
    this.currentPath = '';
    this.startTimer();
  }

  updateSyncProgress(path: string, current: number, total: number) {
    this.syncing = true;
    this.currentPath = path;
    this.filesScanned = current;
    this.filesTotal = total;
    if (!this.timerInterval) {
      this.startTimer();
    }
  }

  completeSync() {
    this.syncing = false;
    this.filesScanned = 0;
    this.filesTotal = 0;
    this.currentPath = '';
    this.stopTimer();
  }

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
