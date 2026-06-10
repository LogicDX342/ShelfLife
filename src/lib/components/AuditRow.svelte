<script lang="ts">
  import { undoAuditEntry } from '$lib/api/triage';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AuditEntry } from '$lib/types';
  import { formatBytes, formatDate, getErrorMessage } from '$lib/utils/format';

  let { entry, onRefresh } = $props<{ entry: AuditEntry; onRefresh: () => Promise<void> }>();
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function undo() {
    busy = true;
    error = null;
    try {
      await undoAuditEntry(entry.id);
      await onRefresh();
    } catch (reason) {
      error = getErrorMessage(reason, i18n.t('audit.errorUndo'));
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
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-4v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
          />
        </svg>
      {:else if entry.action_kind === 'Move'}
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4"
          />
        </svg>
      {:else if entry.action_kind === 'Rename'}
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"
          />
        </svg>
      {:else if entry.action_kind === 'Pin'}
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z"
          />
        </svg>
      {:else if entry.action_kind === 'Snooze'}
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      {:else}
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l18 18"
          />
        </svg>
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
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2.5"
            d="M5 13l4 4L19 7"
          />
        </svg>
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

{#if error}
  <p class="text-xs text-red-500 font-medium px-4 pt-1">{error}</p>
{/if}
