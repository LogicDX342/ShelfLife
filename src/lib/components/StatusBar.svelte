<script lang="ts">
  import { formatBytes } from '$lib/utils/format';
  import type { TrackedFile } from '$lib/types';
  import { i18n } from '$lib/i18n/i18n.svelte';

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
    <span>{i18n.t('status.review')}</span>
    <strong>{reviewCount}</strong>
  </div>
  <div>
    <span>{i18n.t('status.tracked')}</span>
    <strong>{files.length}</strong>
  </div>
  <div>
    <span>{i18n.t('status.recoverableSize')}</span>
    <strong>{formatBytes(totalSize)}</strong>
  </div>
</section>
