<script lang="ts">
  import IconFolder from '@lucide/svelte/icons/folder';
  import IconFolderOpen from '@lucide/svelte/icons/folder-open';
  import IconAlertTriangle from '@lucide/svelte/icons/triangle-alert';
  import { onMount } from 'svelte';

  import { getConfig } from '$lib/api/config';
  import { selectDirectory } from '$lib/api/files';
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
  import type { AppConfig, AppError, TrackedFile, UserTriageAction } from '$lib/types';
  import { cn } from '$lib/utils';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';
  import {
    getDestinationOptions,
    loadRecentMoveDestinations,
    recordRecentMoveDestination,
  } from '$lib/utils/moveDestinations';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import FileCard from './FileCard.svelte';

  let config = $state<AppConfig | null>(null);
  let currentDirectory = $state('');
  let selectedPaths = $state<string[]>([]);
  let bulkErrors = $state<Record<string, AppError>>({});
  let bulkAction = $state<'Move' | 'Pin' | 'Ignore' | 'Snooze' | 'TrashNow'>('Move');
  let bulkSnoozeDays = $state(7);
  let bulkMoveDestination = $state('');
  let bulkDefaultMoveDestination = $state<string | null>(null);
  let bulkRecentMoveDestinations = $state<string[]>([]);
  let bulkPickedMoveDestination = $state<string | null>(null);
  let bulkMoveLoading = $state(false);
  let bulkMoveReady = $state(false);
  let confirmBulk = $state(false);

  let visibleFoldersCount = $state(50);
  let visibleFilesCount = $state(50);
  let bulkMoveDestinationOptions = $derived(
    getDestinationOptions(
      bulkDefaultMoveDestination,
      bulkRecentMoveDestinations,
      bulkPickedMoveDestination,
      {
        default: i18n.t('file.defaultDestination'),
        recent: i18n.t('file.recentDestination'),
        chosen: i18n.t('file.chosenDestination'),
      },
    ),
  );

  // Reset rendering limits on folder navigation
  $effect(() => {
    if (currentDirectory || currentDirectory === '') {
      visibleFoldersCount = 50;
      visibleFilesCount = 50;
    }
  });

  $effect(() => {
    if (selectedPaths.length === 0 || bulkAction !== 'Move') {
      bulkMoveReady = false;
    } else if (!bulkMoveReady) {
      bulkMoveReady = true;
      void refreshBulkMoveDestinations();
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
      ManuallyIgnored: 1,
      RuleIgnored: 1,
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

  let failedSelectedCount = $derived(selectedPaths.filter((p) => p in bulkErrors).length);

  function toggleSelectAll(checked: boolean) {
    if (checked) {
      const pathsToSelect = currentFolderFiles.map((f) => f.path);
      selectedPaths = Array.from(new Set([...selectedPaths, ...pathsToSelect]));
    } else {
      const pathsToRemove = currentFolderFiles.map((f) => f.path);
      selectedPaths = selectedPaths.filter((p) => !pathsToRemove.includes(p));
      for (const p of pathsToRemove) {
        delete bulkErrors[p];
      }
    }
  }

  function selectReviewableInFolder() {
    const pathsToSelect = currentFolderFiles
      .filter((f) => f.state === 'Stale' || f.state === 'Decaying')
      .map((f) => f.path);
    selectedPaths = Array.from(new Set([...selectedPaths, ...pathsToSelect]));
  }

  function setSelected(path: string, selected: boolean) {
    if (!selected) {
      delete bulkErrors[path];
    }
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
          : bulkAction === 'Move'
            ? { Move: { destination_folder: bulkMoveDestination.trim() } }
            : bulkAction;

      for (const p of selectedPaths) {
        delete bulkErrors[p];
      }

      const result = await executeBulkTriageAction(selectedPaths, action);
      if (bulkAction === 'Move' && result.entries.length > 0) {
        recordRecentMoveDestination(bulkMoveDestination);
      }

      for (const failure of result.failures) {
        bulkErrors[failure.path] = failure.error;
      }

      selectedPaths = result.failures.map((f) => f.path);

      const summary = i18n.t('browser.bulkSummary', {
        action: bulkAction,
        succeeded: result.entries.length,
        failed: result.failures.length,
      });

      if (result.failures.length === 0) {
        notifications.success(summary);
      } else if (result.entries.length > 0) {
        notifications.warning(summary);
      } else {
        notifications.error(summary);
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('browser.errorBulkAction')));
    }
  }

  async function refreshBulkMoveDestinations() {
    bulkMoveLoading = true;
    try {
      config ??= await getConfig();
      bulkDefaultMoveDestination = config.default_move_destination;
      bulkRecentMoveDestinations = await loadRecentMoveDestinations(bulkDefaultMoveDestination);
      bulkPickedMoveDestination = null;
      bulkMoveDestination = bulkDefaultMoveDestination ?? bulkRecentMoveDestinations[0] ?? '';
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('file.errorMoveDestinations')));
    } finally {
      bulkMoveLoading = false;
    }
  }

  async function chooseBulkMoveDestination() {
    try {
      const selected = await selectDirectory(
        i18n.t('file.selectMoveDestination'),
        bulkMoveDestination || bulkDefaultMoveDestination || bulkRecentMoveDestinations[0],
      );
      if (selected) {
        bulkPickedMoveDestination = selected;
        bulkMoveDestination = selected;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('file.errorMoveDestinations')));
    }
  }

  function bulkActionLabel() {
    if (bulkAction === 'Move') return i18n.t('file.actionMove');
    if (bulkAction === 'Pin') return i18n.t('file.pin');
    if (bulkAction === 'Ignore') return i18n.t('file.ignore');
    if (bulkAction === 'Snooze') return i18n.t('file.snooze');
    return i18n.t('file.trash');
  }

  function getWorstStateColors(state: string) {
    switch (state) {
      case 'Fresh':
        return 'bg-success';
      case 'Stale':
        return 'bg-warning';
      case 'Decaying':
        return 'bg-destructive';
      case 'Pinned':
        return 'bg-info';
      case 'ManuallyIgnored':
      case 'RuleIgnored':
        return 'bg-muted-foreground';
      default:
        return 'bg-muted-foreground';
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
      <h2 class="text-xs font-bold uppercase tracking-wider text-muted-foreground mb-3">
        {i18n.t('browser.folders')}
      </h2>
      <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
        {#each directoryContents.folders.slice(0, visibleFoldersCount) as folder (folder.path)}
          <Button
            onclick={() => (currentDirectory = folder.path)}
            variant="outline"
            class="group h-auto justify-between p-3 text-left"
          >
            <div class="flex items-center gap-2 min-w-0">
              <!-- Folder Icon -->
              <IconFolder
                data-icon="inline-start"
                class="size-6 group-hover:scale-105 transition-transform"
              />
              <div class="min-w-0">
                <h3 class="text-sm font-semibold truncate text-foreground" title={folder.name}>
                  {folder.name}
                </h3>
                <p class="text-[10px] text-muted-foreground truncate mt-0.5" title={folder.path}>
                  {folder.filesCount === 1
                    ? i18n.t('browser.fileCountSingular', { count: folder.filesCount })
                    : i18n.t('browser.fileCountPlural', { count: folder.filesCount })}
                </p>
              </div>
            </div>

            <!-- Worst state indicator dot -->
            <span class="flex size-2 relative flex-shrink-0">
              {#if folder.worstState === 'Decaying' || folder.worstState === 'Stale'}
                <span
                  class={cn(
                    'animate-ping absolute inline-flex size-full rounded-full opacity-75',
                    getWorstStateColors(folder.worstState),
                  )}
                ></span>
              {/if}
              <span
                class={cn(
                  'relative inline-flex size-2 rounded-full',
                  getWorstStateColors(folder.worstState),
                )}
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
        <h2 class="text-xs font-bold uppercase tracking-wider text-muted-foreground">
          {i18n.t('browser.files')}
        </h2>

        <!-- Bulk Select Operations -->
        <div class="flex items-center gap-3">
          <Button type="button" variant="ghost" size="xs" onclick={selectReviewableInFolder}>
            {i18n.t('browser.selectReviewable')}
          </Button>
          <span class="text-border">|</span>
          <label class="flex items-center gap-1.5 text-xs font-semibold cursor-pointer select-none">
            <Checkbox checked={allSelected} onCheckedChange={toggleSelectAll} />
            {i18n.t('browser.selectAll')}
          </label>
        </div>
      </div>

      <div class="flex flex-col gap-4">
        {#each directoryContents.files.slice(0, visibleFilesCount) as file (file.path)}
          <FileCard
            {file}
            selectable
            selected={selectedPaths.includes(file.path)}
            error={bulkErrors[file.path]
              ? getErrorMessage(bulkErrors[file.path], i18n.t('browser.errorBulkAction'))
              : null}
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

  <!-- Spacer when bulk selection bar is active so content at bottom is not covered -->
  {#if selectedPaths.length > 0}
    <div class="h-32 md:h-30 shrink-0 pointer-events-none" aria-hidden="true"></div>
  {/if}
</PageBody>

<!-- Sticky Bulk Action Bar at Bottom -->
{#if selectedPaths.length > 0}
  <div
    class="fixed bottom-6 left-[88px] right-6 z-30 flex flex-col gap-3 border bg-card/95 p-4 text-card-foreground shadow-lg backdrop-blur-xl animate-slide-up md:left-[264px]"
  >
    {#if failedSelectedCount > 0}
      <div
        class="flex items-center gap-2 rounded-md border border-warning/30 bg-warning/10 px-3 py-2 text-xs font-medium text-warning"
      >
        <IconAlertTriangle class="size-4 shrink-0" />
        <span>{i18n.t('browser.bulkFailuresNotice', { count: failedSelectedCount })}</span>
      </div>
    {/if}

    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-4">
        <div class="flex flex-col">
          <span class="text-sm font-semibold">
            {i18n.t('dashboard.selected', { count: selectedPaths.length })}
          </span>
          <span class="text-xs text-muted-foreground">
            {formatBytes(selectedSize)}
          </span>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          onclick={() => {
            selectedPaths = [];
            bulkErrors = {};
          }}
        >
          {i18n.t('dashboard.clearSelection')}
        </Button>

        <Select.Root type="single" bind:value={bulkAction}>
          <Select.Trigger>
            <span data-slot="select-value">{bulkActionLabel()}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Group>
              <Select.Item value="Move" label={i18n.t('file.actionMove')} />
              <Select.Item value="Pin" label={i18n.t('file.pin')} />
              <Select.Item value="Ignore" label={i18n.t('file.ignore')} />
              <Select.Item value="Snooze" label={i18n.t('file.snooze')} />
              <Select.Item value="TrashNow" label={i18n.t('file.trash')} />
            </Select.Group>
          </Select.Content>
        </Select.Root>
        {#if bulkAction === 'Snooze'}
          <Input min="1" type="number" bind:value={bulkSnoozeDays} class="w-16 text-xs" />
        {/if}

        {#if bulkAction !== 'Move'}
          <Button onclick={() => (confirmBulk = true)}>
            {i18n.t('browser.applyAction')}
          </Button>
        {/if}
      </div>
    </div>

    {#if bulkAction === 'Move'}
      <div class="flex flex-wrap items-center justify-end gap-2 border-t border-border pt-3">
        {#if bulkMoveDestinationOptions.length > 0}
          <Select.Root type="single" bind:value={bulkMoveDestination} disabled={bulkMoveLoading}>
            <Select.Trigger class="min-w-64 flex-1">
              <span data-slot="select-value">
                {bulkMoveDestination || i18n.t('file.chooseDestination')}
              </span>
            </Select.Trigger>
            <Select.Content>
              <Select.Group>
                {#each bulkMoveDestinationOptions as option (option.path)}
                  <Select.Item value={option.path} label={option.path}>
                    <span class="flex flex-col gap-0.5">
                      <span>{option.label}</span>
                      <span class="text-muted-foreground">{option.path}</span>
                    </span>
                  </Select.Item>
                {/each}
              </Select.Group>
            </Select.Content>
          </Select.Root>
        {/if}
        <Button variant="outline" disabled={bulkMoveLoading} onclick={chooseBulkMoveDestination}>
          {i18n.t('file.chooseAnotherFolder')}
        </Button>
        <Button
          disabled={bulkMoveLoading || !bulkMoveDestination.trim()}
          onclick={() => (confirmBulk = true)}
        >
          {i18n.t('file.actionMove')}
        </Button>
      </div>
    {/if}
  </div>
{/if}

<ConfirmDialog
  open={confirmBulk}
  title={i18n.t('dialog.confirmTitle')}
  message={i18n.t('browser.bulkConfirmMsg', {
    action: bulkActionLabel(),
    count: selectedPaths.length,
    size: formatBytes(selectedSize),
  })}
  confirmLabel={i18n.t('browser.apply')}
  onCancel={() => (confirmBulk = false)}
  onConfirm={runBulkAction}
/>
