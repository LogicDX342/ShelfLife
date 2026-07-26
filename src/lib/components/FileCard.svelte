<script lang="ts">
  import IconCircleCheck from '@lucide/svelte/icons/circle-check';
  import IconClock from '@lucide/svelte/icons/clock';
  import IconEyeOff from '@lucide/svelte/icons/eye-off';
  import IconDocument from '@lucide/svelte/icons/file';
  import IconFolderArrowRight from '@lucide/svelte/icons/folder-input';
  import IconFolderOpen from '@lucide/svelte/icons/folder-open';
  import IconPin from '@lucide/svelte/icons/pin';
  import IconDelete from '@lucide/svelte/icons/trash-2';
  import IconAlertTriangle from '@lucide/svelte/icons/triangle-alert';

  import { getConfig } from '$lib/api/config';
  import { explainFile, openFileLocation, selectDirectory } from '$lib/api/files';
  import { confirmRuleAction, executeTriageAction } from '$lib/api/triage';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import * as Select from '$lib/components/ui/select';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type {
    FileDecayState,
    RuleMatchExplanation,
    TrackedFile,
    UserTriageAction,
  } from '$lib/types';
  import { cn } from '$lib/utils';
  import { formatBytes, formatDate, getErrorMessage } from '$lib/utils/format';
  import {
    getDestinationOptions,
    loadRecentMoveDestinations,
    recordRecentMoveDestination,
  } from '$lib/utils/moveDestinations';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import ExplanationBadge from './ExplanationBadge.svelte';

  let {
    file,
    selectable = false,
    selected = false,
    error = null,
    onSelectedChange = () => {},
  } = $props<{
    file: TrackedFile;
    selectable?: boolean;
    selected?: boolean;
    error?: string | null;
    onSelectedChange?: (path: string, selected: boolean) => void;
  }>();

  let explanations = $state<RuleMatchExplanation[]>([]);
  let matchedExplanations = $derived(
    explanations.filter((e) => e.rule_id !== null && e.proposed_action !== null),
  );
  let effectiveExplanation = $derived(
    matchedExplanations.find((explanation) => explanation.mode !== 'PreviewOnly') ?? null,
  );
  let currentTime = $state(Math.floor(Date.now() / 1000));
  let confirmableExplanation = $derived.by(() => {
    if (effectiveExplanation?.mode !== 'AskFirst' || !effectiveExplanation.rule_id) return null;
    if (typeof file.expiry !== 'object' || !('At' in file.expiry)) return null;
    return file.expiry.At <= currentTime ? effectiveExplanation : null;
  });

  $effect(() => {
    if (effectiveExplanation?.mode !== 'AskFirst') return;
    if (typeof file.expiry !== 'object' || !('At' in file.expiry)) return;
    currentTime = Math.floor(Date.now() / 1000);
    const millisecondsUntilExpiry = Math.max(0, (file.expiry.At - currentTime) * 1000);
    if (millisecondsUntilExpiry === 0) return;
    const timer = window.setTimeout(
      () => (currentTime = Math.floor(Date.now() / 1000)),
      Math.min(millisecondsUntilExpiry, 2_147_483_647),
    );
    return () => window.clearTimeout(timer);
  });

  let busy = $state(false);
  let moveOpen = $state(false);
  let snoozeOpen = $state(false);
  let moveLoading = $state(false);
  let moveDestination = $state('');
  let defaultMoveDestination = $state<string | null>(null);
  let recentMoveDestinations = $state<string[]>([]);
  let pickedMoveDestination = $state<string | null>(null);
  let snoozeDays = $state('7');
  let customSnoozeDays = $state(7);
  let pendingAction = $state<UserTriageAction | null>(null);
  let pendingRule = $state<RuleMatchExplanation | null>(null);
  let pendingTitle = $state('');
  let pendingMessage = $state('');

  const snoozeOptions = ['1', '3', '7', '14', '30', '-1'];
  let moveDestinationOptions = $derived(
    getDestinationOptions(defaultMoveDestination, recentMoveDestinations, pickedMoveDestination, {
      default: i18n.t('file.defaultDestination'),
      recent: i18n.t('file.recentDestination'),
      chosen: i18n.t('file.chosenDestination'),
    }),
  );

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
      if (typeof action === 'object' && 'Move' in action) {
        recordRecentMoveDestination(action.Move.destination_folder);
        moveOpen = false;
      }
      if (typeof action === 'object' && 'Snooze' in action) {
        snoozeOpen = false;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('file.errorAction')));
    } finally {
      busy = false;
    }
  }

  function toggleSnoozeControls() {
    snoozeOpen = !snoozeOpen;
    if (snoozeOpen) {
      moveOpen = false;
    }
  }

  function actionName(action: UserTriageAction) {
    if (action === 'Pin') return i18n.t('file.pin');
    if (action === 'Ignore') return i18n.t('file.ignore');
    if (action === 'TrashNow') return i18n.t('file.trash');
    if (typeof action === 'object' && 'Snooze' in action) return i18n.t('file.snooze');
    if (typeof action === 'object' && 'Move' in action) return i18n.t('file.actionMove');
    return i18n.t('file.actionLabel');
  }

  function queueAction(action: UserTriageAction) {
    pendingRule = null;
    pendingAction = action;
    pendingTitle = actionName(action);
    pendingMessage = i18n.t('file.confirmMsg', {
      action: actionName(action),
      name: file.file_name,
    });
  }

  function queueRuleConfirmation(explanation: RuleMatchExplanation) {
    if (!explanation.rule_id || !explanation.proposed_action) return;
    pendingAction = null;
    pendingRule = explanation;
    pendingTitle = i18n.t('file.confirmRuleAction');
    const ruleName = explanation.rule_name ?? i18n.t('file.noRule');
    if (typeof explanation.proposed_action === 'object' && 'Move' in explanation.proposed_action) {
      pendingMessage = i18n.t(
        explanation.proposed_action.Move.rename_template
          ? 'file.confirmMoveRuleMsgWithRename'
          : 'file.confirmMoveRuleMsg',
        {
          destination: explanation.proposed_action.Move.destination_folder,
          rule: ruleName,
          name: file.file_name,
        },
      );
    } else {
      const action = explanation.proposed_action;
      pendingMessage = i18n.t('file.confirmRuleMsg', {
        action:
          action === 'Trash'
            ? i18n.t('file.trash')
            : action === 'Ignore'
              ? i18n.t('file.ignore')
              : i18n.t('file.actionMove'),
        rule: ruleName,
        name: file.file_name,
      });
    }
  }

  async function confirmPendingAction() {
    if (pendingRule?.rule_id) {
      const ruleId = pendingRule.rule_id;
      pendingRule = null;
      busy = true;
      try {
        await confirmRuleAction(file.path, ruleId);
      } catch (reason) {
        notifications.error(getErrorMessage(reason, i18n.t('file.errorAction')));
      } finally {
        busy = false;
      }
      return;
    }
    if (pendingAction) {
      const action = pendingAction;
      pendingAction = null;
      await act(action);
    }
  }

  function snoozeAction() {
    const days = snoozeDays === '-1' ? customSnoozeDays : Number(snoozeDays);
    queueAction({ Snooze: { seconds: Math.max(1, days) * 24 * 60 * 60 } });
  }

  async function toggleMoveControls() {
    if (moveOpen) {
      moveOpen = false;
      return;
    }

    moveOpen = true;
    snoozeOpen = false;
    moveLoading = true;
    try {
      const config = await getConfig();
      defaultMoveDestination = config.default_move_destination;
      recentMoveDestinations = await loadRecentMoveDestinations(defaultMoveDestination);
      pickedMoveDestination = null;
      moveDestination = defaultMoveDestination ?? recentMoveDestinations[0] ?? '';
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('file.errorMoveDestinations')));
    } finally {
      moveLoading = false;
    }
  }

  async function chooseMoveDestination() {
    try {
      const selected = await selectDirectory(
        i18n.t('file.selectMoveDestination'),
        moveDestination || defaultMoveDestination || recentMoveDestinations[0],
      );
      if (selected) {
        pickedMoveDestination = selected;
        moveDestination = selected;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('file.errorMoveDestinations')));
    }
  }

  function queueMoveAction() {
    const destination = moveDestination.trim();
    if (destination) {
      queueAction({ Move: { destination_folder: destination } });
    }
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
        return 'border-l-4 border-l-success';
      case 'Stale':
        return 'border-l-4 border-l-warning';
      case 'Decaying':
        return 'border-l-4 border-l-destructive';
      case 'Pinned':
        return 'border-l-4 border-l-info';
      default:
        return 'border-l-4 border-l-muted-foreground';
    }
  }

  function isIgnored(state: FileDecayState) {
    return state === 'ManuallyIgnored' || state === 'RuleIgnored';
  }
</script>

<Card.Root
  class={cn(
    'relative flex flex-col gap-2 p-3 transition-all duration-200',
    getBorderColor(file.state),
  )}
>
  <!-- Row 1: File Info -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
    <!-- Left side: Checkbox + Icon + Name/Path stack -->
    <div class="flex items-start gap-3 min-w-0 flex-1">
      {#if selectable}
        <Checkbox
          aria-label={`Select ${file.file_name}`}
          checked={selected}
          class="mt-1"
          onCheckedChange={(checked) => onSelectedChange(file.path, checked)}
        />
      {/if}

      <div class="mt-0.5 text-muted-foreground flex-shrink-0">
        <IconDocument class="size-5 opacity-80" />
      </div>

      <div class="min-w-0 flex-1">
        <h3
          class="text-sm font-semibold tracking-tight truncate text-foreground"
          title={file.file_name}
        >
          {file.file_name}
        </h3>
        <div class="flex items-center gap-1.5 mt-0.5">
          <p class="text-xs text-muted-foreground truncate" title={file.path}>
            {file.path}
          </p>
          <Button
            variant="ghost"
            size="icon-xs"
            class="shrink-0"
            onclick={openLocation}
            title={i18n.t('file.openFolder')}
            aria-label={i18n.t('file.openFolder')}
          >
            <IconFolderOpen />
          </Button>
        </div>
      </div>
    </div>

    <!-- Right side: Size, Date, Badge, Chevron -->
    <div class="flex items-center gap-4 justify-between sm:justify-end shrink-0">
      <div class="flex flex-col text-left sm:text-right">
        <span class="text-xs font-semibold text-foreground">
          {formatBytes(file.size_bytes)}
        </span>
        <span class="text-[10px] text-muted-foreground mt-0.5">
          {i18n.t('file.lastUpdated')}: {formatDate(file.freshness_at)}
        </span>
      </div>

      <Badge variant="outline" class="font-medium text-xs px-2.5 py-0.5">
        {i18n.t(isIgnored(file.state) ? 'tab.ignored' : `tab.${file.state.toLowerCase()}`)}
      </Badge>
    </div>
  </div>

  {#if error}
    <div
      class="border-t border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive rounded-md flex items-center gap-2"
    >
      <IconAlertTriangle class="size-4 shrink-0 text-destructive" />
      <span class="font-medium">{error}</span>
    </div>
  {/if}

  <!-- Row 2: Matched Rule -->
  {#if matchedExplanations.length > 0}
    <div class="border-t border-border/40 pt-2.5">
      <div class="flex flex-wrap gap-2 items-center">
        {#each matchedExplanations as explanation (explanation.rule_id)}
          <ExplanationBadge {explanation} />
        {/each}
      </div>
    </div>
  {/if}

  <!-- Row 3: Action Buttons -->
  <div class="flex flex-wrap items-center justify-between gap-3 border-t border-border/40 pt-2.5">
    <!-- Left actions (Pin/Ignore, Move, Snooze) -->
    <div class="flex flex-wrap gap-2">
      {#if file.state === 'Pinned'}
        <Button variant="outline" size="sm" disabled={busy} onclick={() => queueAction('Ignore')}>
          <IconEyeOff data-icon="inline-start" />
          {i18n.t('file.ignore')}
        </Button>
      {:else if isIgnored(file.state)}
        <Button variant="outline" size="sm" disabled={busy} onclick={() => queueAction('Pin')}>
          <IconPin data-icon="inline-start" />
          {i18n.t('file.pin')}
        </Button>
      {:else}
        <Button variant="outline" size="sm" disabled={busy} onclick={() => queueAction('Pin')}>
          <IconPin data-icon="inline-start" />
          {i18n.t('file.pin')}
        </Button>
        <Button variant="outline" size="sm" disabled={busy} onclick={() => queueAction('Ignore')}>
          <IconEyeOff data-icon="inline-start" />
          {i18n.t('file.ignore')}
        </Button>
      {/if}

      <Button
        variant={moveOpen ? 'secondary' : 'outline'}
        size="sm"
        disabled={busy}
        onclick={toggleMoveControls}
      >
        <IconFolderArrowRight data-icon="inline-start" />
        {i18n.t('file.actionMove')}
      </Button>

      <Button
        variant={snoozeOpen ? 'secondary' : 'outline'}
        size="sm"
        disabled={busy}
        onclick={toggleSnoozeControls}
      >
        <IconClock data-icon="inline-start" />
        {i18n.t('file.snooze')}
      </Button>
    </div>

    <!-- Right Action -->
    <div class="flex flex-wrap gap-2">
      {#if confirmableExplanation}
        <Button
          variant="default"
          size="sm"
          disabled={busy}
          onclick={() => queueRuleConfirmation(confirmableExplanation)}
        >
          <IconCircleCheck data-icon="inline-start" />
          {i18n.t('file.confirmRuleAction')}
        </Button>
      {/if}
      <Button
        variant="destructive"
        size="sm"
        disabled={busy}
        onclick={() => queueAction('TrashNow')}
      >
        <IconDelete data-icon="inline-start" />
        {i18n.t('file.trash')}
      </Button>
    </div>
  </div>

  <!-- Inline Move Controls -->
  {#if moveOpen}
    <div
      class="flex flex-col gap-2.5 border-t border-border/40 pt-3 animate-in fade-in slide-in-from-top-1 duration-200"
    >
      <Label
        for="move-destination-{file.file_name}"
        class="text-xs font-semibold text-muted-foreground"
      >
        {i18n.t('file.moveTitle')}
      </Label>
      <div class="flex flex-wrap items-center gap-2">
        {#if moveDestinationOptions.length > 0}
          <Select.Root type="single" bind:value={moveDestination} disabled={moveLoading}>
            <Select.Trigger
              id="move-destination-{file.file_name}"
              class="min-w-64 flex-1 h-9 text-xs"
            >
              <span data-slot="select-value">
                {moveDestination || i18n.t('file.chooseDestination')}
              </span>
            </Select.Trigger>
            <Select.Content
              class="max-w-(--bits-select-anchor-width) transition-[opacity,transform]"
            >
              <Select.Group>
                {#each moveDestinationOptions as option (option.path)}
                  <Select.Item
                    value={option.path}
                    label={option.path}
                    class="[&>span:last-child]:min-w-0"
                  >
                    <span class="min-w-0 flex-1 truncate" title={option.path}>{option.path}</span>
                    {#if option.isDefault}
                      <Badge variant="secondary">{i18n.t('file.default')}</Badge>
                    {/if}
                  </Select.Item>
                {/each}
              </Select.Group>
            </Select.Content>
          </Select.Root>
        {/if}
        <Button
          variant="outline"
          size="sm"
          disabled={moveLoading || busy}
          onclick={chooseMoveDestination}
        >
          {i18n.t('file.chooseAnotherFolder')}
        </Button>
        <Button variant="ghost" size="sm" disabled={busy} onclick={() => (moveOpen = false)}>
          {i18n.t('dialog.cancel')}
        </Button>
        <Button
          size="sm"
          disabled={busy || moveLoading || !moveDestination.trim()}
          onclick={queueMoveAction}
        >
          {i18n.t('file.actionMove')}
        </Button>
      </div>
    </div>
  {/if}

  <!-- Inline Snooze Controls -->
  {#if snoozeOpen}
    <div
      class="flex flex-col gap-2.5 border-t border-border/40 pt-3 animate-in fade-in slide-in-from-top-1 duration-200"
    >
      <Label for="snooze-in-{file.file_name}" class="text-xs font-semibold text-muted-foreground">
        {i18n.t('file.snoozeTitle')}
      </Label>
      <div class="flex flex-wrap items-center gap-2">
        <Select.Root type="single" bind:value={snoozeDays}>
          <Select.Trigger
            id="snooze-in-{file.file_name}"
            class={cn('h-9 text-xs', snoozeDays === '-1' ? 'w-32 shrink-0' : 'flex-1 min-w-64')}
          >
            <span data-slot="select-value">{snoozeLabel(snoozeDays)}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Group>
              {#each snoozeOptions as days (days)}
                <Select.Item value={days} label={snoozeLabel(days)} class="text-xs" />
              {/each}
            </Select.Group>
          </Select.Content>
        </Select.Root>

        {#if snoozeDays === '-1'}
          <Input
            min="1"
            type="number"
            bind:value={customSnoozeDays}
            placeholder={i18n.t('file.days')}
            class="flex-1 min-w-0 h-9 text-xs"
          />
        {/if}

        <Button variant="ghost" size="sm" disabled={busy} onclick={() => (snoozeOpen = false)}>
          {i18n.t('dialog.cancel')}
        </Button>

        <Button size="sm" disabled={busy} onclick={snoozeAction}>
          {i18n.t('file.snooze')}
        </Button>
      </div>
    </div>
  {/if}

  <!-- Modal Confirmation Dialog -->
  <ConfirmDialog
    open={pendingAction !== null || pendingRule !== null}
    title={pendingTitle}
    message={pendingMessage}
    confirmLabel={pendingTitle}
    onCancel={() => {
      pendingAction = null;
      pendingRule = null;
    }}
    onConfirm={confirmPendingAction}
  />
</Card.Root>
