<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { filesState } from '$lib/stores/files.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { getConfig } from '$lib/api/config';
  import { executeBulkTriageAction } from '$lib/api/triage';
  import type { AppConfig, TrackedFile, UserTriageAction } from '$lib/types';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';
  import FileCard from './FileCard.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';

  let config = $state<AppConfig | null>(null);
  let currentDirectory = $state('');
  let selectedPaths = $state<string[]>([]);
  let bulkAction = $state<'MoveToSafeFolder' | 'Pin' | 'Ignore' | 'Snooze' | 'TrashNow'>(
    'MoveToSafeFolder',
  );
  let bulkSnoozeDays = $state(7);
  let confirmBulk = $state(false);
  let bulkError = $state<string | null>(null);
  let bulkSummary = $state<string | null>(null);

  let visibleFoldersCount = $state(50);
  let visibleFilesCount = $state(50);

  // Reset rendering limits on folder navigation
  $effect(() => {
    if (currentDirectory || currentDirectory === '') {
      visibleFoldersCount = 50;
      visibleFilesCount = 50;
    }
  });

  onMount(() => {
    async function init() {
      config = await getConfig();
      filesState.refresh();
    }
    init();

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

  let watchTargets = $derived(
    config ? config.watch_targets.filter((t) => t.enabled).map((t) => t.path) : [],
  );

  function getWorstState(state1: string, state2: string): string {
    const priority: Record<string, number> = {
      Decaying: 5,
      Stale: 4,
      Fresh: 3,
      Pinned: 2,
      Ignored: 1,
      Missing: 0,
    };
    return (priority[state1] || 0) >= (priority[state2] || 0) ? state1 : state2;
  }

  // Parse files and targets into folders and files for the current directory path
  let directoryContents = $derived.by(() => {
    if (!currentDirectory) {
      // Root level: show enabled watch targets as folders
      return {
        folders: watchTargets.map((target) => {
          const targetLower = target.replace(/\\/g, '/').toLowerCase();
          const targetLowerWithSlash = targetLower.endsWith('/') ? targetLower : targetLower + '/';

          const childFiles = filesState.files.filter((f) => {
            const pathNorm = f.path.replace(/\\/g, '/').toLowerCase();
            return pathNorm.startsWith(targetLowerWithSlash) || pathNorm === targetLower;
          });

          let worstState = 'Fresh';
          for (const f of childFiles) {
            worstState = getWorstState(worstState, f.state);
          }

          return {
            name: target.split(/[\\/]/).filter(Boolean).pop() || target,
            path: target,
            isWatchTarget: true,
            filesCount: childFiles.length,
            worstState,
          };
        }),
        files: [],
      };
    }

    const normalizedCurrent = currentDirectory.replace(/\\/g, '/').toLowerCase();
    const prefix = normalizedCurrent.endsWith('/') ? normalizedCurrent : normalizedCurrent + '/';

    const foldersMap: Record<
      string,
      { name: string; path: string; filesCount: number; worstState: string }
    > = {};
    const immediateFiles: TrackedFile[] = [];

    for (const file of filesState.files) {
      const pathNorm = file.path.replace(/\\/g, '/');
      const pathLower = pathNorm.toLowerCase();

      if (pathLower.startsWith(prefix)) {
        const relativePath = pathNorm.substring(prefix.length);
        const parts = relativePath.split('/');

        if (parts.length > 1) {
          // Subdirectory
          const subfolderName = parts[0];
          const separator = currentDirectory.includes('\\') ? '\\' : '/';
          const subfolderPath =
            currentDirectory.endsWith('\\') || currentDirectory.endsWith('/')
              ? currentDirectory + subfolderName
              : currentDirectory + separator + subfolderName;

          const key = subfolderName.toLowerCase();
          let folderData = foldersMap[key];
          if (!folderData) {
            folderData = {
              name: subfolderName,
              path: subfolderPath,
              filesCount: 0,
              worstState: 'Fresh',
            };
            foldersMap[key] = folderData;
          }

          folderData.filesCount += 1;
          folderData.worstState = getWorstState(folderData.worstState, file.state);
        } else if (parts.length === 1 && parts[0] !== '') {
          // Immediate file
          immediateFiles.push(file);
        }
      }
    }

    // Sort folders alphabetically
    const sortedFolders = Object.values(foldersMap).sort((a, b) => a.name.localeCompare(b.name));

    // Sort immediate files by name
    const sortedFiles = immediateFiles.sort((a, b) => a.file_name.localeCompare(b.file_name));

    return {
      folders: sortedFolders,
      files: sortedFiles,
    };
  });

  // Calculate breadcrumbs from current directory
  let breadcrumbs = $derived.by(() => {
    if (!currentDirectory) return [{ name: 'Root', path: '' }];

    const separator = currentDirectory.includes('\\') ? '\\' : '/';
    const parts = currentDirectory.split(/[\\/]/).filter(Boolean);
    const list = [{ name: 'Root', path: '' }];

    let accruedPath = '';
    // Preserve Windows drive letters
    const startsWithDrive = /^[a-zA-Z]:/.test(currentDirectory);

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (i === 0) {
        accruedPath = part;
        if (startsWithDrive && accruedPath.endsWith(':')) {
          accruedPath += separator;
        }
      } else {
        accruedPath += (accruedPath.endsWith(separator) ? '' : separator) + part;
      }
      list.push({ name: part, path: accruedPath });
    }
    return list;
  });

  let selectedSize = $derived(
    filesState.files
      .filter((file) => selectedPaths.includes(file.path))
      .reduce((sum, file) => sum + file.size_bytes, 0),
  );

  let currentFolderFiles = $derived(directoryContents.files);
  let allSelected = $derived(
    currentFolderFiles.length > 0 &&
      currentFolderFiles.every((f) => selectedPaths.includes(f.path)),
  );

  function toggleSelectAll(checked: boolean) {
    if (checked) {
      const pathsToSelect = currentFolderFiles.map((f) => f.path);
      selectedPaths = Array.from(new Set([...selectedPaths, ...pathsToSelect]));
    } else {
      const pathsToRemove = currentFolderFiles.map((f) => f.path);
      selectedPaths = selectedPaths.filter((p) => !pathsToRemove.includes(p));
    }
  }

  function selectReviewableInFolder() {
    const pathsToSelect = currentFolderFiles
      .filter((f) => f.state === 'Stale' || f.state === 'Decaying')
      .map((f) => f.path);
    selectedPaths = Array.from(new Set([...selectedPaths, ...pathsToSelect]));
  }

  function setSelected(path: string, selected: boolean) {
    selectedPaths = selected
      ? Array.from(new Set([...selectedPaths, path]))
      : selectedPaths.filter((item) => item !== path);
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
      bulkError = getErrorMessage(reason, 'Bulk action failed.');
    }
  }

  function getWorstStateColors(state: string) {
    switch (state) {
      case 'Fresh':
        return 'bg-green-500 text-green-500 border-green-500/20';
      case 'Stale':
        return 'bg-amber-500 text-amber-500 border-amber-500/20';
      case 'Decaying':
        return 'bg-red-500 text-red-500 border-red-500/20';
      case 'Pinned':
        return 'bg-blue-500 text-blue-500 border-blue-500/20';
      case 'Ignored':
        return 'bg-neutral-400 text-neutral-400 border-neutral-400/20';
      default:
        return 'bg-neutral-500 text-neutral-500 border-neutral-500/20';
    }
  }
</script>

<div class="h-full flex flex-col min-h-0 relative gap-6">
  <!-- Header & Breadcrumbs -->
  <header
    class="border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0"
  >
    <h1 class="text-2xl font-bold tracking-tight">{i18n.t('nav.browser')}</h1>

    <!-- Breadcrumbs Navigation -->
    <nav
      class="flex items-center flex-wrap gap-1 mt-2 text-sm text-fluent-muted-light dark:text-fluent-muted-dark"
    >
      {#each breadcrumbs as crumb, index (crumb.path)}
        {#if index > 0}
          <span class="opacity-50 mx-1">/</span>
        {/if}
        <button
          onclick={() => (currentDirectory = crumb.path)}
          class="hover:text-fluent-accent hover:underline transition-colors font-medium {index ===
          breadcrumbs.length - 1
            ? 'text-fluent-text-light dark:text-fluent-text-dark font-semibold'
            : ''}"
        >
          {crumb.name}
        </button>
      {/each}
    </nav>
  </header>

  <!-- Error / Success Messages -->
  {#if bulkError}
    <div
      class="p-3 text-sm rounded bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-900/50 flex-shrink-0"
    >
      {bulkError}
    </div>
  {/if}
  {#if bulkSummary}
    <div
      class="p-3 text-sm rounded bg-green-100 dark:bg-green-950/40 text-green-700 dark:text-green-300 border border-green-200 dark:border-green-900/50 flex-shrink-0"
    >
      {bulkSummary}
    </div>
  {/if}

  <!-- Contents List -->
  <div class="flex-1 overflow-y-auto space-y-6 pb-24 pr-1">
    <!-- Folders Section -->
    {#if directoryContents.folders.length > 0}
      <div>
        <h2
          class="text-xs font-bold uppercase tracking-wider text-fluent-muted-light dark:text-fluent-muted-dark mb-3"
        >
          Folders
        </h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
          {#each directoryContents.folders.slice(0, visibleFoldersCount) as folder (folder.path)}
            <button
              onclick={() => (currentDirectory = folder.path)}
              class="fluent-card p-4 flex items-center justify-between text-left hover:border-fluent-accent/50 hover:bg-black/2.5 dark:hover:bg-white/2.5 transition-all select-none group cursor-pointer"
            >
              <div class="flex items-center gap-3 min-w-0">
                <!-- Folder Icon -->
                <svg
                  class="w-8 h-8 text-fluent-accent/80 group-hover:scale-105 transition-transform flex-shrink-0"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                  />
                </svg>
                <div class="min-w-0">
                  <h3
                    class="text-sm font-semibold truncate text-fluent-text-light dark:text-fluent-text-dark"
                    title={folder.name}
                  >
                    {folder.name}
                  </h3>
                  <p
                    class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark truncate mt-0.5"
                    title={folder.path}
                  >
                    {folder.filesCount} file{folder.filesCount === 1 ? '' : 's'}
                  </p>
                </div>
              </div>

              <!-- Worst state indicator dot -->
              <span class="flex h-2 w-2 relative flex-shrink-0">
                {#if folder.worstState === 'Decaying' || folder.worstState === 'Stale'}
                  <span
                    class="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75 {getWorstStateColors(
                      folder.worstState,
                    ).split(' ')[0]}"
                  ></span>
                {/if}
                <span
                  class="relative inline-flex rounded-full h-2 w-2 {getWorstStateColors(
                    folder.worstState,
                  ).split(' ')[0]}"
                ></span>
              </span>
            </button>
          {/each}
        </div>

        {#if directoryContents.folders.length > visibleFoldersCount}
          <div class="pt-4 flex justify-center">
            <button
              type="button"
              class="fluent-button w-full justify-center text-xs font-semibold py-2"
              onclick={() => (visibleFoldersCount += 50)}
            >
              Load More Folders ({directoryContents.folders.length - visibleFoldersCount} remaining)
            </button>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Files Section -->
    {#if directoryContents.files.length > 0}
      <div>
        <div class="flex items-center justify-between mb-3">
          <h2
            class="text-xs font-bold uppercase tracking-wider text-fluent-muted-light dark:text-fluent-muted-dark"
          >
            Files
          </h2>

          <!-- Bulk Select Operations -->
          <div class="flex items-center gap-3">
            <button
              type="button"
              class="text-xs font-semibold text-fluent-accent hover:underline"
              onclick={selectReviewableInFolder}
            >
              Select Reviewable
            </button>
            <span class="text-fluent-border-light dark:text-fluent-border-dark">|</span>
            <label
              class="flex items-center gap-1.5 text-xs font-semibold cursor-pointer select-none"
            >
              <input
                type="checkbox"
                checked={allSelected}
                onchange={(e) => toggleSelectAll(e.currentTarget.checked)}
                class="rounded border-neutral-300 dark:border-neutral-700 text-fluent-accent focus:ring-fluent-accent"
              />
              Select All
            </label>
          </div>
        </div>

        <div class="space-y-4">
          {#each directoryContents.files.slice(0, visibleFilesCount) as file (file.path)}
            <FileCard
              {file}
              onRefresh={() => filesState.refresh()}
              selectable
              selected={selectedPaths.includes(file.path)}
              onSelectedChange={setSelected}
            />
          {/each}
        </div>

        {#if directoryContents.files.length > visibleFilesCount}
          <div class="pt-4 flex justify-center">
            <button
              type="button"
              class="fluent-button w-full justify-center text-xs font-semibold py-2"
              onclick={() => (visibleFilesCount += 50)}
            >
              Load More Files ({directoryContents.files.length - visibleFilesCount} remaining)
            </button>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Empty Folder Screen -->
    {#if directoryContents.folders.length === 0 && directoryContents.files.length === 0}
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
            stroke-width="1.8"
            d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5M5 19a2 2 0 002-2v-5M7 10h3m-3 4h3m4-4h.01M17 14h.01"
          />
        </svg>
        <h3 class="text-base font-semibold">Empty Folder</h3>
        <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
          This directory does not contain any tracked files or folders.
        </p>
      </div>
    {/if}
  </div>

  <!-- Sticky Bulk Action Bar at Bottom -->
  {#if selectedPaths.length > 0}
    <div
      class="fixed bottom-6 left-6 right-6 md:left-[264px] acrylic-card p-4 rounded-lg shadow-lg flex items-center justify-between border border-fluent-accent/30 z-10 animate-slide-up"
    >
      <div class="flex items-center gap-4">
        <div class="flex flex-col">
          <span class="text-sm font-semibold text-fluent-accent">
            {i18n.t('dashboard.selected', { count: selectedPaths.length })}
          </span>
          <span class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark">
            {formatBytes(selectedSize)}
          </span>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <button class="fluent-button" onclick={() => (selectedPaths = [])}>
          {i18n.t('dashboard.clearSelection')}
        </button>

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

        <button
          class="fluent-button fluent-button-primary animate-pulse"
          onclick={() => (confirmBulk = true)}
        >
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
