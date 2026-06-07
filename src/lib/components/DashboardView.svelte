<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { runReconciliationScan } from '$lib/api/config';
  import { executeBulkTriageAction } from '$lib/api/triage';
  import { filesState } from '$lib/stores/files.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { UserTriageAction } from '$lib/types';
  import { formatBytes } from '$lib/utils/format';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import FileList from './FileList.svelte';
  import StatusBar from './StatusBar.svelte';

  let selectedPaths = $state<string[]>([]);
  let bulkAction = $state<'MoveToSafeFolder' | 'Pin' | 'Ignore' | 'Snooze' | 'TrashNow'>(
    'MoveToSafeFolder',
  );
  let bulkSnoozeDays = $state(7);
  let confirmBulk = $state(false);
  let bulkError = $state<string | null>(null);
  let bulkSummary = $state<string | null>(null);
  let isScanning = $state(false);

  const tabs = ['All', 'Stale', 'Decaying', 'Pinned', 'Ignored'] as const;
  let activeTab = $state<(typeof tabs)[number]>('All');
  let searchQuery = $state('');

  let selectedSize = $derived(
    filesState.files
      .filter((file) => selectedPaths.includes(file.path))
      .reduce((sum, file) => sum + file.size_bytes, 0),
  );

  let tabCounts = $derived({
    All: filesState.files.length,
    Stale: filesState.files.filter((f) => f.state === 'Stale').length,
    Decaying: filesState.files.filter((f) => f.state === 'Decaying').length,
    Pinned: filesState.files.filter((f) => f.state === 'Pinned').length,
    Ignored: filesState.files.filter((f) => f.state === 'Ignored').length,
  });

  let filteredFiles = $derived(
    filesState.files.filter((file) => {
      const matchesSearch =
        file.file_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        file.path.toLowerCase().includes(searchQuery.toLowerCase());
      if (!matchesSearch) return false;
      if (activeTab === 'All') return true;
      return file.state.toLowerCase() === activeTab.toLowerCase();
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

  async function reconcile() {
    isScanning = true;
    try {
      await runReconciliationScan();
      await filesState.refresh();
    } catch (e) {
      console.error(e);
    } finally {
      isScanning = false;
    }
  }

  function setSelected(path: string, selected: boolean) {
    selectedPaths = selected
      ? Array.from(new Set([...selectedPaths, path]))
      : selectedPaths.filter((item) => item !== path);
  }

  function selectReviewable() {
    selectedPaths = filteredFiles
      .filter((file) => file.state === 'Stale' || file.state === 'Decaying')
      .map((file) => file.path);
  }

  function selectedBulkAction(): UserTriageAction {
    if (bulkAction === 'Snooze') return { Snooze: { seconds: bulkSnoozeDays * 24 * 60 * 60 } };
    return bulkAction;
  }

  async function runBulkAction() {
    confirmBulk = false;
    bulkError = null;
    bulkSummary = null;
    try {
      const result = await executeBulkTriageAction(selectedPaths, selectedBulkAction());
      bulkSummary =
        i18n.t('dashboard.bulkActions') +
        `: ${result.entries.length} succeeded${result.failures.length ? `, ${result.failures.length} failed` : ''}.`;
      selectedPaths = [];
      await filesState.refresh();
    } catch (reason) {
      bulkError = reason instanceof Error ? reason.message : 'Bulk action failed.';
    }
  }
</script>

<div class="h-full flex flex-col min-h-0 relative gap-6">
  <!-- Header -->
  <header
    class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0"
  >
    <div>
      <h1 class="text-2xl font-bold tracking-tight">{i18n.t('dashboard.title')}</h1>
      <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
        {i18n.t('dashboard.subtitle')}
      </p>
    </div>
    <button class="fluent-button fluent-button-primary" onclick={reconcile} disabled={isScanning}>
      {#if isScanning}
        <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
          ></circle>
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
          ></path>
        </svg>
        Scanning...
      {:else}
        Run Scan
      {/if}
    </button>
  </header>

  <!-- Status Summary Band -->
  <div class="flex-shrink-0">
    <StatusBar files={filesState.files} />
  </div>

  <!-- Search & Tabs Control Bar -->
  <div
    class="flex flex-col md:flex-row gap-4 items-stretch md:items-center justify-between bg-black/5 dark:bg-white/5 p-2 rounded-lg flex-shrink-0"
  >
    <!-- State Tabs -->
    <div class="flex flex-wrap gap-1">
      {#each tabs as tab (tab)}
        <button
          onclick={() => (activeTab = tab)}
          class="px-3 py-1.5 text-xs font-semibold rounded transition-colors relative {activeTab ===
          tab
            ? 'bg-fluent-accent text-white'
            : 'hover:bg-black/5 dark:hover:bg-white/5 text-fluent-muted-light dark:text-fluent-muted-dark'}"
        >
          {i18n.t(`tab.${tab.toLowerCase()}`)}
          <span
            class="ml-1 px-1.5 py-0.2 text-[10px] rounded-full {activeTab === tab
              ? 'bg-white/20 text-white'
              : 'bg-black/10 dark:bg-white/10'}"
          >
            {tabCounts[tab as keyof typeof tabCounts]}
          </span>
        </button>
      {/each}
    </div>

    <!-- Search Input -->
    <div class="relative flex-1 max-w-md">
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
        bind:value={searchQuery}
        class="fluent-input search-input w-full"
      />
    </div>
  </div>

  <!-- Scrollable content -->
  <div class="flex-1 overflow-y-auto space-y-6 pb-24 pr-1">
    <!-- Messages -->
    {#if bulkError}
      <div
        class="p-3 text-sm rounded bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-900/50"
      >
        {bulkError}
      </div>
    {/if}
    {#if bulkSummary}
      <div
        class="p-3 text-sm rounded bg-green-100 dark:bg-green-950/40 text-green-700 dark:text-green-300 border border-green-200 dark:border-green-900/50"
      >
        {bulkSummary}
      </div>
    {/if}

    <!-- Queue Content -->
    {#if filesState.error}
      <div class="p-6 text-center text-red-500">
        <p class="font-semibold">Error loading files</p>
        <p class="text-sm mt-1">{filesState.error}</p>
      </div>
    {:else if filesState.loading}
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
          >Loading files...</span
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
          Try adjusting your filters or watch configurations.
        </p>
      </div>
    {:else}
      <FileList
        files={filteredFiles}
        onRefresh={() => filesState.refresh()}
        {selectedPaths}
        onSelectedChange={setSelected}
      />
    {/if}
  </div>

  <!-- Sticky Bulk Action Bar at Bottom of Queue -->
  {#if selectedPaths.length > 0}
    <div
      class="fixed bottom-6 left-6 right-6 md:left-[264px] acrylic-card p-4 rounded-lg shadow-lg flex items-center justify-between border border-fluent-accent/30 z-10 animate-slide-up"
    >
      <div class="flex items-center gap-4">
        <div class="flex flex-col">
          <span class="text-sm font-semibold text-fluent-accent"
            >{i18n.t('dashboard.selected', { count: selectedPaths.length })}</span
          >
          <span class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark"
            >{formatBytes(selectedSize)}</span
          >
        </div>
      </div>
      <div class="flex items-center gap-2">
        <button class="fluent-button" onclick={selectReviewable}>Select Reviewable</button>
        <button class="fluent-button" onclick={() => (selectedPaths = [])}
          >{i18n.t('dashboard.clearSelection')}</button
        >

        <select bind:value={bulkAction} class="fluent-input text-xs">
          <option value="MoveToSafeFolder">MoveToSafeFolder</option>
          <option value="Pin">Pin</option>
          <option value="Ignore">Ignore</option>
          <option value="Snooze">Snooze</option>
          <option value="TrashNow">TrashNow</option>
        </select>
        {#if bulkAction === 'Snooze'}
          <input
            min="1"
            type="number"
            bind:value={bulkSnoozeDays}
            class="fluent-input w-16 text-xs"
          />
        {/if}

        <button class="fluent-button fluent-button-primary" onclick={() => (confirmBulk = true)}>
          Apply Action
        </button>
      </div>
    </div>
  {/if}
</div>

<ConfirmDialog
  open={confirmBulk}
  title={i18n.t('dialog.confirmTitle')}
  message={`${bulkAction} will be applied to ${selectedPaths.length} files totaling ${formatBytes(selectedSize)}. Each changed file will create its own audit row.`}
  confirmLabel="Apply"
  onCancel={() => (confirmBulk = false)}
  onConfirm={runBulkAction}
/>

<style>
  .search-input {
    padding-left: 2.25rem;
  }

  @keyframes slide-up {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
  .animate-slide-up {
    animation: slide-up 0.25s cubic-bezier(0.25, 0.8, 0.25, 1) forwards;
  }
</style>
