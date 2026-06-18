<script lang="ts">
  import IconDocument from '@lucide/svelte/icons/file';
  import IconSearch from '@lucide/svelte/icons/search';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';

  import { Button } from '$lib/components/ui/button';
  import * as Empty from '$lib/components/ui/empty';
  import * as InputGroup from '$lib/components/ui/input-group';
  import { Spinner } from '$lib/components/ui/spinner';
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
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      searchQuery = searchInputValue;
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

  onMount(() => {
    filesState.refresh();
    const refresh = () => filesState.refresh();
    window.addEventListener('focus', refresh);

    let active = true;
    let unlistenReconciliation: (() => void) | null = null;
    let unlistenAction: (() => void) | null = null;

    listen('reconciliation_completed', refresh).then((unlisten) => {
      if (active) unlistenReconciliation = unlisten;
      else unlisten();
    });
    listen('action_completed', refresh).then((unlisten) => {
      if (active) unlistenAction = unlisten;
      else unlisten();
    });

    return () => {
      active = false;
      window.removeEventListener('focus', refresh);
      unlistenReconciliation?.();
      unlistenAction?.();
    };
  });
</script>

<div class="h-full flex flex-col min-h-0 relative gap-6">
  <!-- Header -->
  <header
    class="flex flex-col md:flex-row md:items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0 gap-4"
  >
    <div>
      <h1 class="text-2xl font-bold tracking-tight">{i18n.t('nav.queue')}</h1>
      <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
        {i18n.t('dashboard.subtitle')}
      </p>
    </div>

    <!-- Search Input -->
    <div>
      <InputGroup.Root>
        <InputGroup.Input
          type="text"
          placeholder={i18n.t('dashboard.search')}
          bind:value={searchInputValue}
        />
        <InputGroup.Addon>
          <IconSearch />
        </InputGroup.Addon>
      </InputGroup.Root>
    </div>
  </header>

  <!-- Status Summary Band -->
  <div class="flex-shrink-0">
    <StatusBar files={filesState.files} />
  </div>

  <!-- Scrollable content -->
  <div class="flex-1 overflow-y-auto space-y-4 pb-16 pr-1">
    {#if filesState.error}
      <div class="p-6 text-center text-red-500">
        <p class="font-semibold">{i18n.t('dashboard.errorLoading')}</p>
        <p class="text-sm mt-1">{filesState.error}</p>
      </div>
    {:else if filesState.loading && filesState.files.length === 0}
      <div class="py-12 flex flex-col items-center justify-center gap-3">
        <Spinner class="h-8 w-8 text-primary" />
        <span class="text-sm text-muted-foreground">{i18n.t('dashboard.loadingQueue')}</span>
      </div>
    {:else if filteredFiles.length === 0}
      <Empty.Root class="border bg-muted/30">
        <Empty.Header>
          <Empty.Media>
            <IconDocument
              class="h-12 w-12 text-fluent-muted-light dark:text-fluent-muted-dark opacity-50"
            />
          </Empty.Media>
          <Empty.Title>{i18n.t('dashboard.noFiles')}</Empty.Title>
          <Empty.Description>
            {i18n.t('dashboard.noFilesDesc')}
          </Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <div class="space-y-4">
        {#each filteredFiles.slice(0, visibleLimit) as file (file.path)}
          <FileCard {file} onRefresh={() => filesState.refresh()} selectable={false} />
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
  </div>
</div>
