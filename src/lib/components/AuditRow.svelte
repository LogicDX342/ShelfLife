<script lang="ts">
  import IconCheckmark from '@lucide/svelte/icons/check';
  import IconClock from '@lucide/svelte/icons/clock';
  import IconEyeOff from '@lucide/svelte/icons/eye-off';
  import IconFolderArrowRight from '@lucide/svelte/icons/folder-input';
  import IconEdit from '@lucide/svelte/icons/pencil';
  import IconPin from '@lucide/svelte/icons/pin';
  import IconDelete from '@lucide/svelte/icons/trash-2';

  import { undoAuditEntry } from '$lib/api/triage';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type { AuditEntry } from '$lib/types';
  import { formatBytes, formatDate, getErrorMessage } from '$lib/utils/format';

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

<Card.Root>
  <Card.Header class="items-center">
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
          <Card.Title class="text-sm font-semibold tracking-tight">
            {entry.file_name}
          </Card.Title>
          <Badge variant="secondary">
            {entry.action_kind}
          </Badge>
        </div>
        <Card.Description class="text-xs truncate" title={entry.source_path}>
          {i18n.t('audit.source', { path: entry.source_path })}
        </Card.Description>
        {#if entry.destination_path}
          <Card.Description class="text-xs truncate" title={entry.destination_path}>
            {i18n.t('audit.destLabel', { path: entry.destination_path })}
          </Card.Description>
        {/if}
        <div class="flex items-center gap-2 pt-1 text-[10px] text-muted-foreground">
          <span>{i18n.t('audit.size', { size: formatBytes(entry.size_bytes) })}</span>
          <span>•</span>
          <span>{formatDate(entry.timestamp)}</span>
          {#if entry.rule_name}
            <span>•</span>
            <span class="font-medium text-primary">
              {i18n.t('audit.rule', { name: entry.rule_name })}
            </span>
          {/if}
        </div>
      </div>
    </div>

    <!-- Right: Undo Action -->
    <Card.Action
      class="flex flex-col items-end gap-1 flex-shrink-0 self-stretch sm:self-center justify-between sm:justify-center"
    >
      {#if undoAvailable}
        <Button variant="outline" onclick={undo} disabled={busy}>
          {#if busy}
            {i18n.t('audit.undoing')}
          {:else}
            {i18n.t('audit.undo')}
          {/if}
        </Button>
      {:else if entry.undo_status === 'Completed'}
        <Button
          variant="outline"
          disabled
          class="border-green-200 dark:border-green-900/40 bg-green-50 dark:bg-green-950/20 text-green-600 dark:text-green-400 disabled:opacity-100 disabled:bg-green-50 dark:disabled:bg-green-950/20 disabled:text-green-600 dark:disabled:text-green-400 disabled:border-green-200 dark:disabled:border-green-900/40"
        >
          <IconCheckmark class="w-4 h-4" />
          {i18n.t('audit.undone')}
        </Button>
      {:else}
        <span
          class="text-[10px] max-w-[150px] text-right text-muted-foreground italic truncate"
          title={statusText}
        >
          {statusText}
        </span>
      {/if}
    </Card.Action>
  </Card.Header>
</Card.Root>
