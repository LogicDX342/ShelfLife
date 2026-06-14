<script lang="ts">
  import { explainFile, openFileLocation } from '$lib/api/files';
  import { executeTriageAction } from '$lib/api/triage';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { RuleMatchExplanation, TrackedFile, UserTriageAction } from '$lib/types';
  import { formatBytes, formatDate, getErrorMessage } from '$lib/utils/format';
  import ExplanationBadge from './ExplanationBadge.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import IconDocument from '~icons/fluent/document-24-regular';
  import IconChevronDown from '~icons/fluent/chevron-down-24-regular';
  import IconPin from '~icons/fluent/pin-16-regular';
  import IconFolderArrowRight from '~icons/fluent/folder-arrow-right-16-regular';
  import IconDelete from '~icons/fluent/delete-16-regular';

  let {
    file,
    onRefresh,
    selectable = false,
    selected = false,
    onSelectedChange = () => {},
  } = $props<{
    file: TrackedFile;
    onRefresh: () => Promise<void>;
    selectable?: boolean;
    selected?: boolean;
    onSelectedChange?: (path: string, selected: boolean) => void;
  }>();

  let explanations = $state<RuleMatchExplanation[]>([]);
  let matchedExplanations = $derived(
    explanations.filter((e) => e.rule_id !== null && e.proposed_action !== null),
  );

  let busy = $state(false);
  let expanded = $state(false);
  let renameTemplate = $state('');
  let moveDestination = $state('');
  let snoozeDays = $state(7);
  let customSnoozeDays = $state(7);
  let pendingAction = $state<UserTriageAction | null>(null);
  let pendingTitle = $state('');
  let pendingMessage = $state('');

  const snoozeOptions = [1, 3, 7, 14, 30, -1];

  $effect(() => {
    const path = file.path;
    explainFile(path)
      .then((data) => {
        explanations = data;
      })
      .catch((reason) => {
        notifications.error(getErrorMessage(reason, i18n.t('file.errorExplanation')));
      });
  });

  async function act(action: UserTriageAction) {
    busy = true;
    try {
      await executeTriageAction(file.path, action);
      await onRefresh();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('file.errorAction')));
    } finally {
      busy = false;
    }
  }

  function actionName(action: UserTriageAction) {
    if (action === 'Pin') return i18n.t('file.pin');
    if (action === 'Ignore') return i18n.t('file.ignore');
    if (action === 'MoveToSafeFolder') return i18n.t('file.safeFolder');
    if (action === 'TrashNow') return i18n.t('file.trash');
    if (typeof action === 'object' && 'Snooze' in action) return i18n.t('file.snooze');
    if (typeof action === 'object' && 'Rename' in action) return i18n.t('file.actionRename');
    if (typeof action === 'object' && 'Move' in action) return i18n.t('file.actionMove');
    return i18n.t('file.actionLabel');
  }

  function queueAction(action: UserTriageAction) {
    pendingAction = action;
    pendingTitle = actionName(action);
    pendingMessage = i18n.t('file.confirmMsg', {
      action: actionName(action),
      name: file.file_name,
    });
  }

  async function confirmPendingAction() {
    if (!pendingAction) return;
    const action = pendingAction;
    pendingAction = null;
    await act(action);
  }

  function snoozeAction() {
    const days = snoozeDays === -1 ? customSnoozeDays : snoozeDays;
    queueAction({ Snooze: { seconds: Math.max(1, days) * 24 * 60 * 60 } });
  }

  async function openLocation() {
    try {
      await openFileLocation(file.path);
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('file.errorOpenLocation')));
    }
  }

  // Visual classes based on file status
  function getBorderColor(state: string) {
    switch (state) {
      case 'Fresh':
        return 'border-l-4 border-l-green-500';
      case 'Stale':
        return 'border-l-4 border-l-amber-500';
      case 'Decaying':
        return 'border-l-4 border-l-red-500';
      case 'Pinned':
        return 'border-l-4 border-l-blue-500';
      default:
        return 'border-l-4 border-l-neutral-400';
    }
  }

  function getPillBg(state: string) {
    switch (state) {
      case 'Fresh':
        return 'bg-green-100 text-green-700 dark:bg-green-950/40 dark:text-green-300 border border-green-200 dark:border-green-900/50';
      case 'Stale':
        return 'bg-amber-100 text-amber-700 dark:bg-amber-950/40 dark:text-amber-300 border border-amber-200 dark:border-amber-900/50';
      case 'Decaying':
        return 'bg-red-100 text-red-700 dark:bg-red-950/40 dark:text-red-300 border border-red-200 dark:border-red-900/50';
      case 'Pinned':
        return 'bg-blue-100 text-blue-700 dark:bg-blue-950/40 dark:text-blue-300 border border-blue-200 dark:border-blue-900/50';
      default:
        return 'bg-neutral-100 text-neutral-700 dark:bg-neutral-800/40 dark:text-neutral-300 border border-neutral-200 dark:border-neutral-700/50';
    }
  }
</script>

<article
  class="fluent-card p-4 flex flex-col gap-2 relative overflow-hidden transition-all duration-200 {getBorderColor(
    file.state,
  )}"
>
  <!-- Main Grid Info -->
  <div class="grid grid-cols-1 md:grid-cols-12 gap-4 items-center">
    <!-- Left: Checkbox + Name/Path -->
    <div class="md:col-span-7 flex items-start gap-3 min-w-0">
      {#if selectable}
        <input
          aria-label={`Select ${file.file_name}`}
          checked={selected}
          type="checkbox"
          class="mt-1.5 h-4.5 w-4.5 rounded border-neutral-300 dark:border-neutral-700 text-fluent-accent focus:ring-fluent-accent cursor-pointer"
          onchange={(event) => onSelectedChange(file.path, event.currentTarget.checked)}
        />
      {/if}

      <!-- File type icon representation -->
      <div class="mt-0.5 text-fluent-muted-light dark:text-fluent-muted-dark flex-shrink-0">
        <IconDocument class="w-6 h-6 opacity-75" />
      </div>

      <div class="min-w-0 flex-1">
        <h3
          class="text-sm font-semibold tracking-tight truncate text-fluent-text-light dark:text-fluent-text-dark"
          title={file.file_name}
        >
          {file.file_name}
        </h3>
        <p
          class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark truncate mt-0.5"
          title={file.path}
        >
          {file.path}
        </p>
      </div>
    </div>

    <!-- Right: Size + Freshness + Status Pill -->
    <div
      class="md:col-span-5 flex flex-wrap md:flex-nowrap items-center justify-between md:justify-end gap-4"
    >
      <div class="flex flex-col text-left md:text-right">
        <span class="text-xs font-semibold text-fluent-text-light dark:text-fluent-text-dark"
          >{formatBytes(file.size_bytes)}</span
        >
        <span class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark mt-0.5">
          {i18n.t('file.firstSeen')}: {formatDate(file.first_seen_at)}
        </span>
      </div>

      <div class="flex items-center gap-3">
        <span class="px-2.5 py-0.5 text-xs font-semibold rounded-full {getPillBg(file.state)}">
          {i18n.t(`tab.${file.state.toLowerCase()}`)}
        </span>

        <!-- Toggle chevron for details -->
        <button
          onclick={() => (expanded = !expanded)}
          class="fluent-button p-1 min-w-[32px] h-[32px] rounded-full border border-neutral-200 dark:border-neutral-800"
          aria-label="Toggle details"
        >
          <IconChevronDown
            class="w-4 h-4 transform transition-transform duration-200 {expanded
              ? 'rotate-180'
              : ''}"
          />
        </button>
      </div>
    </div>
  </div>

  <!-- Explanations badges if file matches rules -->
  {#if matchedExplanations.length > 0}
    <div
      class="flex flex-wrap gap-2 border-t border-fluent-border-light dark:border-fluent-border-dark pt-2"
    >
      {#each matchedExplanations as explanation (explanation.rule_id)}
        <ExplanationBadge {explanation} />
      {/each}
    </div>
  {/if}

  <!-- Action buttons row -->
  <div
    class="flex flex-wrap gap-2 border-t border-fluent-border-light dark:border-fluent-border-dark pt-2"
  >
    {#if file.state === 'Pinned'}
      <button class="fluent-button" disabled={busy} onclick={() => queueAction('Ignore')}>
        {i18n.t('file.ignore')}
      </button>
    {:else if file.state === 'Ignored'}
      <button class="fluent-button" disabled={busy} onclick={() => queueAction('Pin')}>
        {i18n.t('file.pin')}
      </button>
    {:else}
      <button
        class="fluent-button text-blue-600 dark:text-blue-400"
        disabled={busy}
        onclick={() => queueAction('Pin')}
      >
        <IconPin class="w-3.5 h-3.5 mr-1" />
        {i18n.t('file.pin')}
      </button>
      <button class="fluent-button" disabled={busy} onclick={() => queueAction('Ignore')}>
        {i18n.t('file.ignore')}
      </button>
    {/if}

    <button class="fluent-button" disabled={busy} onclick={() => queueAction('MoveToSafeFolder')}>
      <IconFolderArrowRight class="w-3.5 h-3.5 mr-1" />
      {i18n.t('file.safeFolder')}
    </button>

    <button
      class="fluent-button text-red-600 dark:text-red-400"
      disabled={busy}
      onclick={() => queueAction('TrashNow')}
    >
      <IconDelete class="w-3.5 h-3.5 mr-1" />
      {i18n.t('file.trash')}
    </button>

    <button class="fluent-button" type="button" onclick={openLocation}>
      {i18n.t('file.openFolder')}
    </button>
  </div>

  <!-- Expandable Details / Forms Panel -->
  {#if expanded}
    <section
      class="border-t border-fluent-border-light dark:border-fluent-border-dark pt-4 space-y-4 animate-expand"
    >
      <!-- Action Forms -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <!-- Rename Form -->
        <form
          class="flex flex-col gap-1.5"
          onsubmit={(event) => {
            event.preventDefault();
            if (renameTemplate.trim()) {
              queueAction({ Rename: { template: renameTemplate } });
            }
          }}
        >
          <label
            for="rename-in-{file.file_name}"
            class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('file.renameTitle')}</label
          >
          <div class="flex gap-2">
            <input
              id="rename-in-{file.file_name}"
              bind:value={renameTemplate}
              placeholder={i18n.t('file.renamePlaceholder')}
              class="fluent-input flex-1 text-xs min-w-0"
            />
            <button
              class="fluent-button text-xs font-semibold"
              disabled={busy || !renameTemplate.trim()}
              type="submit">{i18n.t('file.actionRename')}</button
            >
          </div>
        </form>

        <!-- Move Form -->
        <form
          class="flex flex-col gap-1.5"
          onsubmit={(event) => {
            event.preventDefault();
            if (moveDestination.trim()) {
              queueAction({ Move: { destination_path: moveDestination } });
            }
          }}
        >
          <label
            for="move-in-{file.file_name}"
            class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('file.moveTitle')}</label
          >
          <div class="flex gap-2">
            <input
              id="move-in-{file.file_name}"
              bind:value={moveDestination}
              placeholder={i18n.t('file.movePlaceholder')}
              class="fluent-input flex-1 text-xs min-w-0"
            />
            <button
              class="fluent-button text-xs font-semibold"
              disabled={busy || !moveDestination.trim()}
              type="submit">{i18n.t('file.actionMove')}</button
            >
          </div>
        </form>
      </div>

      <!-- Snooze Form -->
      <form
        class="flex flex-col gap-1.5 max-w-md"
        onsubmit={(event) => {
          event.preventDefault();
          snoozeAction();
        }}
      >
        <label
          for="snooze-in-{file.file_name}"
          class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('file.snoozeTitle')}</label
        >
        <div class="flex gap-2">
          <select
            id="snooze-in-{file.file_name}"
            bind:value={snoozeDays}
            class="fluent-input flex-1 text-xs min-w-0"
          >
            {#each snoozeOptions as days (days)}
              <option value={days}
                >{days === -1
                  ? i18n.t('file.snoozeCustom')
                  : `${days} ${days === 1 ? i18n.t('file.day') : i18n.t('file.days')}`}</option
              >
            {/each}
          </select>
          {#if snoozeDays === -1}
            <input
              min="1"
              type="number"
              bind:value={customSnoozeDays}
              class="fluent-input w-24 text-xs"
              placeholder={i18n.t('file.days')}
            />
          {/if}
          <button class="fluent-button text-xs font-semibold" disabled={busy} type="submit"
            >{i18n.t('file.snooze')}</button
          >
        </div>
      </form>
    </section>
  {/if}

  <!-- Modal Confirmation Dialog -->
  <ConfirmDialog
    open={pendingAction !== null}
    title={pendingTitle}
    message={pendingMessage}
    confirmLabel={pendingTitle}
    onCancel={() => (pendingAction = null)}
    onConfirm={confirmPendingAction}
  />
</article>

<style>
  @keyframes expand {
    from {
      opacity: 0;
      transform: translateY(-5px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .animate-expand {
    animation: expand 0.2s cubic-bezier(0.25, 0.8, 0.25, 1) forwards;
  }
</style>
