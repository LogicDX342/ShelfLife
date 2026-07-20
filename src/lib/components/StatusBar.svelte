<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Separator } from '$lib/components/ui/separator';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { TrackedFile } from '$lib/types';
  import { formatBytes } from '$lib/utils/format';

  let { files = [] } = $props<{ files: TrackedFile[] }>();
  let reviewableFiles = $derived(
    files.filter((file: TrackedFile) => file.state === 'Stale' || file.state === 'Decaying'),
  );
  let reviewCount = $derived(reviewableFiles.length);
  let recoverableSize = $derived(
    reviewableFiles.reduce((sum: number, file: TrackedFile) => sum + file.size_bytes, 0),
  );
</script>

<Card.Root aria-label="Review summary">
  <Card.Content class="grid gap-4 sm:grid-cols-[1fr_auto_1fr_auto_1fr] sm:items-center">
    <div class="min-w-0">
      <span class="text-xs font-medium text-muted-foreground">{i18n.t('status.review')}</span>
      <strong class="mt-1 block text-2xl font-semibold">{reviewCount}</strong>
    </div>
    <Separator orientation="vertical" />
    <div class="min-w-0">
      <span class="text-xs font-medium text-muted-foreground">{i18n.t('status.tracked')}</span>
      <strong class="mt-1 block text-2xl font-semibold">{files.length}</strong>
    </div>
    <Separator orientation="vertical" />
    <div class="min-w-0">
      <span class="text-xs font-medium text-muted-foreground">
        {i18n.t('status.recoverableSize')}
      </span>
      <strong class="mt-1 block truncate text-2xl font-semibold">
        {formatBytes(recoverableSize)}
      </strong>
    </div>
  </Card.Content>
</Card.Root>
