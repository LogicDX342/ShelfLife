<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { filesState } from '$lib/stores/files.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
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
    <div class="relative w-full md:max-w-md">
      <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
        <svg
          class="h-4 w-4 text-fluent-muted-light dark:text-fluent-muted-dark"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
          ></path>
        </svg>
      </div>
      <input
        type="text"
        placeholder={i18n.t('dashboard.search')}
        bind:value={searchInputValue}
        class="fluent-input pl-10 w-full"
      />
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
        <svg class="animate-spin h-8 w-8 text-fluent-accent" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
          ></circle>
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
          ></path>
        </svg>
        <span class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('dashboard.loadingQueue')}</span
        >
      </div>
    {:else if filteredFiles.length === 0}
      <div class="fluent-card py-16 text-center">
        <svg
          class="mx-auto h-12 w-12 text-fluent-muted-light dark:text-fluent-muted-dark opacity-50 mb-3"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="1"
            d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
          ></path>
        </svg>
        <h3 class="text-base font-semibold">{i18n.t('dashboard.noFiles')}</h3>
        <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
          {i18n.t('dashboard.noFilesDesc')}
        </p>
      </div>
    {:else}
      <div class="space-y-4">
        {#each filteredFiles.slice(0, visibleLimit) as file (file.path)}
          <FileCard {file} onRefresh={() => filesState.refresh()} selectable={false} />
        {/each}

        {#if filteredFiles.length > visibleLimit}
          <div class="pt-4 flex justify-center">
            <button
              type="button"
              class="fluent-button w-full justify-center text-xs font-semibold py-2.5"
              onclick={() => (visibleLimit += 100)}
            >
              {i18n.t('dashboard.loadMore', { count: filteredFiles.length - visibleLimit })}
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
