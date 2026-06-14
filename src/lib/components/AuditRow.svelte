<script lang="ts">
  import { undoAuditEntry } from '$lib/api/triage';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AuditEntry } from '$lib/types';
  import { formatBytes, formatDate, getErrorMessage } from '$lib/utils/format';
  import { notifications } from '$lib/stores/notifications.svelte';
  import IconDelete from '~icons/fluent/delete-20-regular';
  import IconFolderArrowRight from '~icons/fluent/folder-arrow-right-20-regular';
  import IconEdit from '~icons/fluent/edit-20-regular';
  import IconPin from '~icons/fluent/pin-20-regular';
  import IconClock from '~icons/fluent/clock-20-regular';
  import IconEyeOff from '~icons/fluent/eye-off-20-regular';
  import IconCheckmark from '~icons/fluent/checkmark-16-regular';

  let { entry, onRefresh } = $props<{ entry: AuditEntry; onRefresh: () => Promise<void> }>();
  let busy = $state(false);

  async function undo() {
    busy = true;
    try {
      await undoAuditEntry(entry.id);
      await onRefresh();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('audit.errorUndo')));
    } finally {
      busy = false;
    }
  }

  let undoAvailable = $derived(entry.undo_status === 'Available');

  // Derive undo status explanation
  let statusText = $derived(
    typeof entry.undo_status === 'string'
      ? entry.undo_status
      : 'Unavailable' in entry.undo_status
        ? entry.undo_status.Unavailable.reason
        : 'Failed' in entry.undo_status
          ? entry.undo_status.Failed.reason
          : 'Unknown',
  );

  function getActionColors(kind: string) {
    switch (kind) {
      case 'Trash':
        return 'text-red-500 bg-red-100 dark:bg-red-950/30';
      case 'Move':
        return 'text-blue-500 bg-blue-100 dark:bg-blue-950/30';
      case 'Rename':
        return 'text-purple-500 bg-purple-100 dark:bg-purple-950/30';
      case 'Pin':
        return 'text-green-500 bg-green-100 dark:bg-green-950/30';
      case 'Snooze':
        return 'text-amber-500 bg-amber-100 dark:bg-amber-950/30';
      case 'Ignore':
        return 'text-neutral-500 bg-neutral-100 dark:bg-neutral-850';
      default:
        return 'text-indigo-500 bg-indigo-100 dark:bg-indigo-950/30';
    }
  }
</script>

<div
  class="fluent-card p-4 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 bg-fluent-card-light dark:bg-fluent-card-dark"
>
  <div class="flex items-start gap-3 min-w-0 flex-1">
    <!-- Action icon badge -->
    <div class="p-2.5 rounded-lg flex-shrink-0 {getActionColors(entry.action_kind)}">
      {#if entry.action_kind === 'Trash'}
        <IconDelete class="w-5 h-5" />
      {:else if entry.action_kind === 'Move'}
        <IconFolderArrowRight class="w-5 h-5" />
      {:else if entry.action_kind === 'Rename'}
        <IconEdit class="w-5 h-5" />
      {:else if entry.action_kind === 'Pin'}
        <IconPin class="w-5 h-5" />
      {:else if entry.action_kind === 'Snooze'}
        <IconClock class="w-5 h-5" />
      {:else}
        <IconEyeOff class="w-5 h-5" />
      {/if}
    </div>

    <!-- Info details -->
    <div class="min-w-0 flex-1 space-y-0.5">
      <div class="flex items-center gap-2">
        <span
          class="text-sm font-semibold tracking-tight text-fluent-text-light dark:text-fluent-text-dark"
          >{entry.file_name}</span
        >
        <span
          class="text-[10px] px-1.5 py-0.2 rounded bg-black/5 dark:bg-white/5 font-semibold text-fluent-muted-light dark:text-fluent-muted-dark uppercase tracking-wider"
          >{entry.action_kind}</span
        >
      </div>
      <p
        class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark truncate"
        title={entry.source_path}
      >
        {i18n.t('audit.source', { path: entry.source_path })}
      </p>
      {#if entry.destination_path}
        <p
          class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark truncate"
          title={entry.destination_path}
        >
          {i18n.t('audit.destLabel', { path: entry.destination_path })}
        </p>
      {/if}
      <div
        class="flex items-center gap-2 pt-1 text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark"
      >
        <span>{i18n.t('audit.size', { size: formatBytes(entry.size_bytes) })}</span>
        <span>•</span>
        <span>{formatDate(entry.timestamp)}</span>
        {#if entry.rule_name}
          <span>•</span>
          <span class="font-medium text-fluent-accent"
            >{i18n.t('audit.rule', { name: entry.rule_name })}</span
          >
        {/if}
      </div>
    </div>
  </div>

  <!-- Right: Undo Action -->
  <div
    class="flex flex-col items-end gap-1 flex-shrink-0 self-stretch sm:self-center justify-between sm:justify-center"
  >
    {#if undoAvailable}
      <button class="fluent-button text-xs font-bold px-3 py-1.5" onclick={undo} disabled={busy}>
        {#if busy}
          {i18n.t('audit.undoing')}
        {:else}
          {i18n.t('audit.undo')}
        {/if}
      </button>
    {:else if entry.undo_status === 'Completed'}
      <span
        class="inline-flex items-center gap-1 text-xs text-green-600 dark:text-green-400 font-semibold px-2.5 py-1 bg-green-50 dark:bg-green-950/20 border border-green-200 dark:border-green-900/40 rounded"
      >
        <IconCheckmark class="w-3.5 h-3.5" />
        {i18n.t('audit.undone')}
      </span>
    {:else}
      <span
        class="text-[10px] max-w-[150px] text-right text-fluent-muted-light dark:text-fluent-muted-dark italic truncate"
        title={statusText}
      >
        {statusText}
      </span>
    {/if}
  </div>
</div>
