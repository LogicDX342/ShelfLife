<script lang="ts">
  import IconFolder from '@lucide/svelte/icons/folder';
  import IconFolderOpen from '@lucide/svelte/icons/folder-open';
  import { onMount } from 'svelte';

  import { getConfig } from '$lib/api/config';
  import { executeBulkTriageAction } from '$lib/api/triage';
  import EmptyState from '$lib/components/common/EmptyState.svelte';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import * as Select from '$lib/components/ui/select';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { filesState } from '$lib/stores/files.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type { AppConfig, TrackedFile, UserTriageAction } from '$lib/types';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import FileCard from './FileCard.svelte';

  let config = $state<AppConfig | null>(null);
  let currentDirectory = $state('');
  let selectedPaths = $state<string[]>([]);
  let bulkAction = $state<'MoveToSafeFolder' | 'Pin' | 'Ignore' | 'Snooze' | 'TrashNow'>(
    'MoveToSafeFolder',
  );
  let bulkSnoozeDays = $state(7);
  let confirmBulk = $state(false);

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
    }
    init();
  });

  let watchTargets = $derived(config ? config.watch_targets.filter((t) => t.enabled) : []);

  function getWorstState(state1: string, state2: string): string {
    const priority: Record<string, number> = {
      Decaying: 5,
      Stale: 4,
      Fresh: 3,
      Pinned: 2,
      Ignored: 1,
    };
    return (priority[state1] || 0) >= (priority[state2] || 0) ? state1 : state2;
  }

  // Parse files and targets into folders and files for the current directory path
  let directoryContents = $derived.by(() => {
    if (!currentDirectory) {
      // Root level: show enabled watch targets as folders
      return {
        folders: watchTargets.map((target) => {
          const childFiles = filesState.files.filter((f) => {
            return f.watch_target_id === target.id;
          });

          let worstState = 'Fresh';
          for (const f of childFiles) {
            worstState = getWorstState(worstState, f.state);
          }

          let watchTargetDisplayName =
            target.path.split(/[\\/]/).filter(Boolean).pop() || target.path;

          return {
            name: watchTargetDisplayName,
            path: target.path,
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

  async function runBulkAction() {
    confirmBulk = false;
    try {
      const action: UserTriageAction =
        bulkAction === 'Snooze'
          ? { Snooze: { seconds: bulkSnoozeDays * 24 * 60 * 60 } }
          : bulkAction;
      const result = await executeBulkTriageAction(selectedPaths, action);
      const summary = i18n.t('browser.bulkSummary', {
        action: bulkAction,
        succeeded: result.entries.length,
        failed: result.failures.length,
      });
      notifications.success(summary);
      selectedPaths = [];
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('browser.errorBulkAction')));
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

<PageHeader title={i18n.t('nav.browser')}>
  {#snippet extra()}
    <!-- Breadcrumbs Navigation -->
    <Breadcrumb.Root class="mt-2">
      <Breadcrumb.List>
        {#each breadcrumbs as crumb, index (crumb.path)}
          {#if index > 0}
            <Breadcrumb.Separator />
          {/if}
          <Breadcrumb.Item>
            {#if index === breadcrumbs.length - 1}
              <Breadcrumb.Page class="font-semibold text-foreground">
                {crumb.name}
              </Breadcrumb.Page>
            {:else}
              <Breadcrumb.Link
                href=""
                onclick={(e) => {
                  e.preventDefault();
                  currentDirectory = crumb.path;
                }}
                class="cursor-pointer"
              >
                {crumb.name}
              </Breadcrumb.Link>
            {/if}
          </Breadcrumb.Item>
        {/each}
      </Breadcrumb.List>
    </Breadcrumb.Root>
  {/snippet}
</PageHeader>

<!-- Contents List -->
<PageBody>
  <!-- Folders Section -->
  {#if directoryContents.folders.length > 0}
    <div>
      <h2
        class="text-xs font-bold uppercase tracking-wider text-fluent-muted-light dark:text-fluent-muted-dark mb-3"
      >
        {i18n.t('browser.folders')}
      </h2>
      <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
        {#each directoryContents.folders.slice(0, visibleFoldersCount) as folder (folder.path)}
          <Button
            onclick={() => (currentDirectory = folder.path)}
            variant="outline"
            class="group h-auto justify-between p-4 text-left"
          >
            <div class="flex items-center gap-3 min-w-0">
              <!-- Folder Icon -->
              <IconFolder
                class="w-8 h-8 text-primary/80 group-hover:scale-105 transition-transform flex-shrink-0"
              />
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
                  {folder.filesCount === 1
                    ? i18n.t('browser.fileCountSingular', { count: folder.filesCount })
                    : i18n.t('browser.fileCountPlural', { count: folder.filesCount })}
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
          </Button>
        {/each}
      </div>

      {#if directoryContents.folders.length > visibleFoldersCount}
        <div class="pt-4 flex justify-center">
          <Button
            type="button"
            variant="outline"
            class="w-full"
            onclick={() => (visibleFoldersCount += 50)}
          >
            {i18n.t('browser.loadMoreFolders', {
              count: directoryContents.folders.length - visibleFoldersCount,
            })}
          </Button>
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
          {i18n.t('browser.files')}
        </h2>

        <!-- Bulk Select Operations -->
        <div class="flex items-center gap-3">
          <Button
            type="button"
            variant="link"
            class="h-auto p-0"
            onclick={selectReviewableInFolder}
          >
            {i18n.t('browser.selectReviewable')}
          </Button>
          <span class="text-fluent-border-light dark:text-fluent-border-dark">|</span>
          <label class="flex items-center gap-1.5 text-xs font-semibold cursor-pointer select-none">
            <Checkbox checked={allSelected} onclick={() => toggleSelectAll(!allSelected)} />
            {i18n.t('browser.selectAll')}
          </label>
        </div>
      </div>

      <div class="space-y-4">
        {#each directoryContents.files.slice(0, visibleFilesCount) as file (file.path)}
          <FileCard
            {file}
            selectable
            selected={selectedPaths.includes(file.path)}
            onSelectedChange={setSelected}
          />
        {/each}
      </div>

      {#if directoryContents.files.length > visibleFilesCount}
        <div class="pt-4 flex justify-center">
          <Button
            type="button"
            variant="outline"
            class="w-full"
            onclick={() => (visibleFilesCount += 50)}
          >
            {i18n.t('browser.loadMoreFiles', {
              count: directoryContents.files.length - visibleFilesCount,
            })}
          </Button>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Empty Folder Screen -->
  {#if directoryContents.folders.length === 0 && directoryContents.files.length === 0}
    <EmptyState
      icon={IconFolderOpen}
      title={i18n.t('browser.emptyFolder')}
      description={i18n.t('browser.emptyFolderDesc')}
    />
  {/if}
</PageBody>

<!-- Sticky Bulk Action Bar at Bottom -->
{#if selectedPaths.length > 0}
  <div
    class="fixed bottom-6 left-[88px] right-6 flex items-center justify-between border bg-card/95 p-4 text-card-foreground shadow-lg backdrop-blur-xl animate-slide-up md:left-[264px]"
  >
    <div class="flex items-center gap-4">
      <div class="flex flex-col">
        <span class="text-sm font-semibold text-primary">
          {i18n.t('dashboard.selected', { count: selectedPaths.length })}
        </span>
        <span class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark">
          {formatBytes(selectedSize)}
        </span>
      </div>
    </div>
    <div class="flex items-center gap-2">
      <Button variant="outline" onclick={() => (selectedPaths = [])}>
        {i18n.t('dashboard.clearSelection')}
      </Button>

      <Select.Root type="single" bind:value={bulkAction}>
        <Select.Trigger>
          <span data-slot="select-value">{bulkAction}</span>
        </Select.Trigger>
        <Select.Content>
          <Select.Group>
            <Select.Item value="MoveToSafeFolder" label="MoveToSafeFolder" />
            <Select.Item value="Pin" label="Pin" />
            <Select.Item value="Ignore" label="Ignore" />
            <Select.Item value="Snooze" label="Snooze" />
            <Select.Item value="TrashNow" label="TrashNow" />
          </Select.Group>
        </Select.Content>
      </Select.Root>
      {#if bulkAction === 'Snooze'}
        <Input min="1" type="number" bind:value={bulkSnoozeDays} class="w-16 text-xs" />
      {/if}

      <Button onclick={() => (confirmBulk = true)}>
        {i18n.t('browser.applyAction')}
      </Button>
    </div>
  </div>
{/if}

<ConfirmDialog
  open={confirmBulk}
  title={i18n.t('dialog.confirmTitle')}
  message={i18n.t('browser.bulkConfirmMsg', {
    action: bulkAction,
    count: selectedPaths.length,
    size: formatBytes(selectedSize),
  })}
  confirmLabel={i18n.t('browser.apply')}
  onCancel={() => (confirmBulk = false)}
  onConfirm={runBulkAction}
/>
