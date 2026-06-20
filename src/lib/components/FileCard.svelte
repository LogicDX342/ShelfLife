<script lang="ts">
  import IconChevronDown from '@lucide/svelte/icons/chevron-down';
  import IconDocument from '@lucide/svelte/icons/file';
  import IconFolderArrowRight from '@lucide/svelte/icons/folder-input';
  import IconPin from '@lucide/svelte/icons/pin';
  import IconDelete from '@lucide/svelte/icons/trash-2';

  import { explainFile, openFileLocation } from '$lib/api/files';
  import { executeTriageAction } from '$lib/api/triage';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import * as Select from '$lib/components/ui/select';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type { RuleMatchExplanation, TrackedFile, UserTriageAction } from '$lib/types';
  import { formatBytes, formatDate, getErrorMessage } from '$lib/utils/format';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import ExplanationBadge from './ExplanationBadge.svelte';

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
  let moveDestination = $state('');
  let snoozeDays = $state('7');
  let customSnoozeDays = $state(7);
  let pendingAction = $state<UserTriageAction | null>(null);
  let pendingTitle = $state('');
  let pendingMessage = $state('');

  const snoozeOptions = ['1', '3', '7', '14', '30', '-1'];

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
    const days = snoozeDays === '-1' ? customSnoozeDays : Number(snoozeDays);
    queueAction({ Snooze: { seconds: Math.max(1, days) * 24 * 60 * 60 } });
  }

  function snoozeLabel(days: string) {
    return days === '-1'
      ? i18n.t('file.snoozeCustom')
      : `${days} ${days === '1' ? i18n.t('file.day') : i18n.t('file.days')}`;
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
</script>

<Card.Root class="relative gap-3 p-4 transition-all duration-200 {getBorderColor(file.state)}">
  <!-- Main Grid Info -->
  <div class="grid grid-cols-1 md:grid-cols-12 gap-4 items-center">
    <!-- Left: Checkbox + Name/Path -->
    <div class="md:col-span-7 flex items-start gap-3 min-w-0">
      {#if selectable}
        <Checkbox
          aria-label={`Select ${file.file_name}`}
          checked={selected}
          class="mt-1.5"
          onclick={() => onSelectedChange(file.path, !selected)}
        />
      {/if}

      <!-- File type icon representation -->
      <div class="mt-0.5 text-fluent-muted-light dark:text-fluent-muted-dark flex-shrink-0">
        <IconDocument class="opacity-75" />
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
        <Badge variant="outline">
          {i18n.t(`tab.${file.state.toLowerCase()}`)}
        </Badge>

        <!-- Toggle chevron for details -->
        <Button
          onclick={() => (expanded = !expanded)}
          variant="outline"
          aria-label="Toggle details"
        >
          <IconChevronDown
            class="w-4 h-4 transform transition-transform duration-200 {expanded
              ? 'rotate-180'
              : ''}"
          />
        </Button>
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
      <Button variant="outline" disabled={busy} onclick={() => queueAction('Ignore')}>
        {i18n.t('file.ignore')}
      </Button>
    {:else if file.state === 'Ignored'}
      <Button variant="outline" disabled={busy} onclick={() => queueAction('Pin')}>
        {i18n.t('file.pin')}
      </Button>
    {:else}
      <Button variant="outline" disabled={busy} onclick={() => queueAction('Pin')}>
        <IconPin />
        {i18n.t('file.pin')}
      </Button>
      <Button variant="outline" disabled={busy} onclick={() => queueAction('Ignore')}>
        {i18n.t('file.ignore')}
      </Button>
    {/if}

    <Button variant="outline" disabled={busy} onclick={() => queueAction('MoveToSafeFolder')}>
      <IconFolderArrowRight />
      {i18n.t('file.safeFolder')}
    </Button>

    <Button variant="destructive" disabled={busy} onclick={() => queueAction('TrashNow')}>
      <IconDelete />
      {i18n.t('file.trash')}
    </Button>

    <Button variant="ghost" type="button" onclick={openLocation}>
      {i18n.t('file.openFolder')}
    </Button>
  </div>

  <!-- Expandable Details -->
  {#if expanded}
    <section
      class="border-t border-fluent-border-light dark:border-fluent-border-dark pt-4 space-y-4 animate-expand"
    >
      <!-- Actions -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <!-- Move -->
        <div class="flex flex-col gap-1.5">
          <Label for="move-in-{file.file_name}">
            {i18n.t('file.moveTitle')}
          </Label>
          <div class="flex gap-2">
            <Input
              id="move-in-{file.file_name}"
              bind:value={moveDestination}
              placeholder={i18n.t('file.movePlaceholder')}
            />
            <Button
              variant="outline"
              disabled={busy || !moveDestination.trim()}
              onclick={() => queueAction({ Move: { destination_folder: moveDestination } })}
            >
              {i18n.t('file.actionMove')}
            </Button>
          </div>
        </div>
        <!-- Snooze -->
        <div class="flex flex-col gap-1.5">
          <Label for="snooze-in-{file.file_name}">
            {i18n.t('file.snoozeTitle')}
          </Label>
          <div class="flex gap-2">
            <Select.Root type="single" bind:value={snoozeDays}>
              <Select.Trigger id="snooze-in-{file.file_name}" class="flex-1 min-w-0">
                <span data-slot="select-value">{snoozeLabel(snoozeDays)}</span>
              </Select.Trigger>
              <Select.Content>
                {#each snoozeOptions as days (days)}
                  <Select.Item value={days} label={snoozeLabel(days)} />
                {/each}
              </Select.Content>
            </Select.Root>
            {#if snoozeDays === '-1'}
              <Input
                min="1"
                type="number"
                bind:value={customSnoozeDays}
                placeholder={i18n.t('file.days')}
              />
            {/if}
            <Button variant="outline" disabled={busy} onclick={snoozeAction}>
              {i18n.t('file.snooze')}
            </Button>
          </div>
        </div>
      </div>
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
</Card.Root>
