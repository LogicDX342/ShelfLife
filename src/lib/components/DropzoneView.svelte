<script lang="ts">
  import IconEyeOff from '@lucide/svelte/icons/eye-off';
  import IconFolderInput from '@lucide/svelte/icons/folder-input';
  import IconLoader from '@lucide/svelte/icons/loader-circle';
  import IconDelete from '@lucide/svelte/icons/trash-2';
  import IconX from '@lucide/svelte/icons/x';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';

  import {
    executeDropzoneIngest,
    executeDropzoneRuleGroup,
    hideDropzone,
    previewDropzoneFiles,
  } from '$lib/api/dropzone';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { DropzoneActionResult, DropzonePreview, DropzoneRuleGroup } from '$lib/types';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';

  let preview = $state<DropzonePreview | null>(null);
  let result = $state<DropzoneActionResult | null>(null);
  let loading = $state(false);
  let executingKey = $state<string | null>(null);
  let errorMessage = $state('');

  let droppedPaths = $derived(preview?.files.map((file) => file.path) ?? []);
  let totalSize = $derived(preview?.files.reduce((sum, file) => sum + file.size_bytes, 0) ?? 0);
  let totalFailures = $derived(
    (result?.failures.length ?? 0) + (preview?.rejected_files.length ?? 0),
  );

  onMount(() => {
    document.body.classList.add('dropzone-body');
    const currentWindow = getCurrentWindow();
    const unlisten = currentWindow.onDragDropEvent((event) => {
      if (event.payload.type === 'drop') {
        void loadPreview(event.payload.paths);
      }
    });

    return () => {
      document.body.classList.remove('dropzone-body');
      void unlisten.then((cleanup) => cleanup());
    };
  });

  async function loadPreview(paths: string[]) {
    loading = true;
    errorMessage = '';
    result = null;
    try {
      preview = await previewDropzoneFiles(paths);
    } catch (reason) {
      preview = null;
      errorMessage = getErrorMessage(reason, i18n.t('dropzone.previewFailed'));
    } finally {
      loading = false;
    }
  }

  async function moveToTarget(targetId: string) {
    if (!preview) return;
    executingKey = `target:${targetId}`;
    errorMessage = '';
    try {
      result = await executeDropzoneIngest(droppedPaths, targetId);
      await finishAction(result);
    } catch (reason) {
      errorMessage = getErrorMessage(reason, i18n.t('dropzone.actionFailed'));
    } finally {
      executingKey = null;
    }
  }

  async function runRuleGroup(group: DropzoneRuleGroup) {
    executingKey = `rule:${group.rule_id}`;
    errorMessage = '';
    try {
      result = await executeDropzoneRuleGroup(group.rule_id, group.file_paths);
      await finishAction(result);
    } catch (reason) {
      errorMessage = getErrorMessage(reason, i18n.t('dropzone.ruleFailed'));
    } finally {
      executingKey = null;
    }
  }

  async function finishAction(actionResult: DropzoneActionResult) {
    const completedPaths = new Set(actionResult.entries.map((entry) => entry.source_path));
    const remainingPaths = droppedPaths.filter((path) => !completedPaths.has(path));

    if (remainingPaths.length === 0) {
      await finishChoice();
      return;
    }

    preview = await previewDropzoneFiles(remainingPaths);
  }

  async function finishChoice() {
    preview = null;
    result = null;
    await hideDropzone();
  }

  async function closeDropzone() {
    preview = null;
    result = null;
    errorMessage = '';
    await hideDropzone();
  }
</script>

<svelte:head>
  <title>{i18n.t('dropzone.titleWindow')}</title>
</svelte:head>

<div
  class="h-screen w-screen overflow-hidden bg-transparent p-2 text-foreground"
  role="application"
  aria-label="ShelfLife dropzone"
>
  <section
    class="h-full rounded-lg border border-border bg-background/95 shadow-lg backdrop-blur flex flex-col gap-2 p-3"
  >
    <header class="flex items-center justify-between gap-2">
      <div class="min-w-0 flex items-center gap-2">
        <div class="min-w-0">
          <h1 class="text-sm font-semibold leading-tight truncate">{i18n.t('dropzone.title')}</h1>
          <p class="text-[11px] text-muted-foreground leading-tight">
            {#if preview}
              {preview.files.length === 1
                ? i18n.t('dropzone.subtitle.file', { count: 1, size: formatBytes(totalSize) })
                : i18n.t('dropzone.subtitle.files', {
                    count: preview.files.length,
                    size: formatBytes(totalSize),
                  })}
            {:else if loading}
              {i18n.t('dropzone.subtitle.reading')}
            {:else}
              {i18n.t('dropzone.subtitle.prompt')}
            {/if}
          </p>
        </div>
      </div>
      <Button
        variant="ghost"
        size="icon"
        aria-label={i18n.t('dropzone.close')}
        onclick={closeDropzone}
      >
        <IconX />
      </Button>
    </header>

    {#if loading}
      <div class="flex-1 grid place-items-center text-sm text-muted-foreground">
        <Spinner />
      </div>
    {:else if errorMessage}
      <div class="rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs">
        {errorMessage}
      </div>
    {:else if preview}
      <div class="min-h-0 flex-1 overflow-y-auto flex flex-col gap-3 pr-1">
        {#if preview.watch_targets.length > 0}
          <div class="flex flex-col gap-1.5">
            <p class="text-[11px] font-semibold text-muted-foreground">
              {i18n.t('dropzone.watchTargets')}
            </p>
            {#each preview.watch_targets as target (target.id)}
              <Button
                variant="outline"
                class="justify-start h-auto min-h-9"
                disabled={executingKey !== null || droppedPaths.length === 0}
                onclick={() => moveToTarget(target.id)}
              >
                {#if executingKey === `target:${target.id}`}
                  <IconLoader data-icon="inline-start" class="animate-spin" />
                {:else}
                  <IconFolderInput data-icon="inline-start" />
                {/if}
                <span class="truncate">{target.path}</span>
              </Button>
            {/each}
          </div>
        {/if}

        {#if preview.rule_groups.length > 0}
          <div class="flex flex-col gap-1.5">
            <p class="text-[11px] font-semibold text-muted-foreground">
              {i18n.t('dropzone.ruleGroups')}
            </p>
            {#each preview.rule_groups as group (group.rule_id)}
              <Button
                variant="secondary"
                class="flex flex-col items-stretch h-auto p-3 w-full text-left gap-1"
                disabled={executingKey !== null}
                onclick={() => runRuleGroup(group)}
              >
                <div class="flex items-center gap-2 w-full min-w-0">
                  {#if executingKey === `rule:${group.rule_id}`}
                    <IconLoader
                      data-icon="inline-start"
                      class="w-4 h-4 animate-spin text-muted-foreground shrink-0"
                    />
                  {:else if group.action === 'Trash'}
                    <IconDelete
                      data-icon="inline-start"
                      class="w-4 h-4 text-destructive shrink-0"
                    />
                  {:else if typeof group.action === 'object' && 'Move' in group.action}
                    <IconFolderInput
                      data-icon="inline-start"
                      class="w-4 h-4 text-primary shrink-0"
                    />
                  {:else if group.action === 'Ignore'}
                    <IconEyeOff
                      data-icon="inline-start"
                      class="w-4 h-4 text-muted-foreground shrink-0"
                    />
                  {/if}
                  <span class="min-w-0 flex-1 font-medium truncate">{group.rule_name}</span>
                  <Badge variant="outline" class="shrink-0">{group.file_count}</Badge>
                </div>

                <div
                  class="text-[10px] text-muted-foreground flex items-center gap-1.5 pl-6 min-w-0"
                >
                  {#if group.action === 'Trash'}
                    <span class="text-destructive font-semibold uppercase tracking-wider shrink-0"
                      >{i18n.t('dropzone.actionTrash')}</span
                    >
                  {:else if typeof group.action === 'object' && 'Move' in group.action}
                    <span
                      class="text-primary font-semibold truncate"
                      title={group.action.Move.destination_folder}
                    >
                      {i18n.t('dropzone.actionMove', {
                        path: group.action.Move.destination_folder,
                      })}
                    </span>
                  {:else if group.action === 'Ignore'}
                    <span class="font-semibold uppercase tracking-wider shrink-0"
                      >{i18n.t('dropzone.actionIgnore')}</span
                    >
                  {/if}
                </div>
              </Button>
            {/each}
          </div>
        {/if}

        {#if preview.preview_only.length > 0 || preview.unmatched_files.length > 0 || totalFailures > 0}
          <div class="flex flex-wrap gap-1.5 text-[11px]">
            {#if preview.preview_only.length > 0}
              <Badge variant="secondary">
                {i18n.t('dropzone.badge.preview', { count: preview.preview_only.length })}
              </Badge>
            {/if}
            {#if preview.unmatched_files.length > 0}
              <Badge variant="secondary">
                {i18n.t('dropzone.badge.unmatched', { count: preview.unmatched_files.length })}
              </Badge>
            {/if}
            {#if totalFailures > 0}
              <Badge variant="destructive">
                {i18n.t('dropzone.badge.failed', { count: totalFailures })}
              </Badge>
            {/if}
          </div>
        {/if}

        {#if result}
          <div class="rounded-md border border-border bg-muted/50 p-2 text-[11px]">
            {i18n.t('dropzone.resultSummary', {
              completed: result.entries.length,
              failed: result.failures.length,
            })}
          </div>
        {/if}
      </div>
    {:else}
      <div
        class="flex-1 rounded-md border border-dashed border-border grid place-items-center text-center px-4 text-xs text-muted-foreground"
      >
        {i18n.t('dropzone.subtitle.prompt')}
      </div>
    {/if}
  </section>
</div>
