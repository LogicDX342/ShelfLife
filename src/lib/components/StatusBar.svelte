<script lang="ts">
  import { formatBytes } from '$lib/utils/format';
  import type { TrackedFile } from '$lib/types';

  let { files = [] } = $props<{ files: TrackedFile[] }>();
  let totalSize = $derived(
    files.reduce((sum: number, file: TrackedFile) => sum + file.size_bytes, 0),
  );
  let reviewCount = $derived(
    files.filter((file: TrackedFile) => file.state === 'Stale' || file.state === 'Decaying').length,
  );
</script>

<section class="status-band" aria-label="Review summary">
  <div>
    <span>Review</span>
    <strong>{reviewCount}</strong>
  </div>
  <div>
    <span>Tracked</span>
    <strong>{files.length}</strong>
  </div>
  <div>
    <span>Recoverable Size</span>
    <strong>{formatBytes(totalSize)}</strong>
  </div>
</section>
