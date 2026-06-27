<script lang="ts">
  import IconDocument from '@lucide/svelte/icons/file';
  import IconSearch from '@lucide/svelte/icons/search';

  import EmptyState from '$lib/components/common/EmptyState.svelte';
  import LoadingState from '$lib/components/common/LoadingState.svelte';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as InputGroup from '$lib/components/ui/input-group';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { filesState } from '$lib/stores/files.svelte';

  import FileCard from './FileCard.svelte';
  import StatusBar from './StatusBar.svelte';

  let searchInputValue = $state('');
  let searchQuery = $state('');
  let visibleLimit = $state(50);

  // Debounce search query
  let debounceTimer: ReturnType<typeof setTimeout>;
  $effect(() => {
    const query = searchInputValue;
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      searchQuery = query;
      visibleLimit = 50; // reset scroll limit when search query changes
    }, 200);
    return () => clearTimeout(debounceTimer);
  });

  // Filter only Stale and Decaying files
  let reviewFiles = $derived(
    filesState.files.filter((file) => file.state === 'Stale' || file.state === 'Decaying'),
  );

  // Sort by expiry (urgency)
  let sortedFiles = $derived(
    [...reviewFiles].sort((a, b) => {
      const getExpiryTime = (expiry: typeof a.expiry) => {
        if (typeof expiry === 'object') {
          if ('At' in expiry) return expiry.At;
          if ('SnoozedUntil' in expiry) return expiry.SnoozedUntil;
        }
        return Infinity; // Permanent
      };
      return getExpiryTime(a.expiry) - getExpiryTime(b.expiry);
    }),
  );

  // Filter sorted files by search query
  let filteredFiles = $derived(
    sortedFiles.filter((file) => {
      if (!searchQuery) return true;
      const q = searchQuery.toLowerCase();
      return file.file_name.toLowerCase().includes(q) || file.path.toLowerCase().includes(q);
    }),
  );
</script>

<PageHeader title={i18n.t('nav.queue')} subtitle={i18n.t('dashboard.subtitle')}>
  {#snippet actions()}
    <!-- Search Input -->
    <InputGroup.Root>
      <InputGroup.Input
        type="text"
        placeholder={i18n.t('dashboard.search')}
        bind:value={searchInputValue}
      />
      <InputGroup.Addon align="inline-end">
        {i18n.t('dashboard.results', { count: filteredFiles.length })}
      </InputGroup.Addon>
      <InputGroup.Addon>
        <IconSearch />
      </InputGroup.Addon>
    </InputGroup.Root>
  {/snippet}
</PageHeader>

<!-- Status Summary Band -->
<div class="w-full flex-shrink-0 px-6 md:px-10 mt-6">
  <div class="max-w-6xl mx-auto w-full">
    <StatusBar files={filesState.files} />
  </div>
</div>

<!-- Scrollable content -->
<PageBody>
  {#if filesState.error}
    <div class="p-6 text-center text-red-500">
      <p class="font-semibold">{i18n.t('dashboard.errorLoading')}</p>
      <p class="text-sm mt-1">{filesState.error}</p>
    </div>
  {:else if filesState.loading && filesState.files.length === 0}
    <LoadingState label={i18n.t('dashboard.loadingQueue')} />
  {:else if filteredFiles.length === 0}
    <EmptyState
      icon={IconDocument}
      title={i18n.t('dashboard.noFiles')}
      description={i18n.t('dashboard.noFilesDesc')}
    />
  {:else}
    <div class="space-y-4">
      {#each filteredFiles.slice(0, visibleLimit) as file (file.path)}
        <FileCard {file} selectable={false} />
      {/each}

      {#if filteredFiles.length > visibleLimit}
        <div class="pt-4 flex justify-center">
          <Button
            type="button"
            variant="outline"
            class="w-full"
            onclick={() => (visibleLimit += 100)}
          >
            {i18n.t('dashboard.loadMore', { count: filteredFiles.length - visibleLimit })}
          </Button>
        </div>
      {/if}
    </div>
  {/if}
</PageBody>
