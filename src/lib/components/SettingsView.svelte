<script lang="ts">
  import IconCheckmark from '@lucide/svelte/icons/check';
  import IconCircleHelp from '@lucide/svelte/icons/circle-help';
  import IconArrowSync from '@lucide/svelte/icons/refresh-cw';
  import { onMount } from 'svelte';

  import {
    getConfig,
    runReconciliationScan,
    saveConfig,
    updateWatchTargets,
  } from '$lib/api/config';
  import { selectDirectory } from '$lib/api/files';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as InputGroup from '$lib/components/ui/input-group';
  import * as Item from '$lib/components/ui/item';
  import { Label } from '$lib/components/ui/label';
  import * as Select from '$lib/components/ui/select';
  import { Spinner } from '$lib/components/ui/spinner';
  import { Switch } from '$lib/components/ui/switch';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { filesState } from '$lib/stores/files.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type { AppConfig, CloseBehavior, WatchTarget } from '$lib/types';
  import { getErrorMessage } from '$lib/utils/format';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import DecayTimelineSlider from './DecayTimelineSlider.svelte';
  import WatchTargetCard from './WatchTargetCard.svelte';

  let config = $state<AppConfig | null>(null);
  let targetToRemove = $state<WatchTarget | null>(null);
  let targetPath = $state('');
  let defaultMoveDestination = $state('');
  let sliderValue = $state([5, 29, 30]);
  let notificationsEnabled = $state(true);
  let startAtLogin = $state(false);
  let dropzoneEnabled = $state(false);
  let closeBehavior = $state<CloseBehavior>('Ask');
  let addingTarget = $state(false);
  let showSavedIndicator = $state(false);
  let savedTimeoutId: number | null = null;

  function createWatchTarget(path: string): WatchTarget {
    return {
      id: crypto.randomUUID(),
      path,
      enabled: true,
      recursive: false,
      ignore_patterns: [],
    };
  }

  async function browseDefaultMoveDestination() {
    try {
      const selected = await selectDirectory(
        i18n.t('settings.selectDefaultMoveDestination'),
        defaultMoveDestination,
      );
      if (selected) {
        defaultMoveDestination = selected;
        await savePreferences();
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorSelectFolder')));
    }
  }

  async function browseTargetFolder() {
    try {
      const selected = await selectDirectory('Select Watch Target Folder', targetPath);
      if (selected) {
        targetPath = selected;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorSelectFolder')));
    }
  }

  onMount(() => {
    void refreshConfig();

    const syncCloseBehavior = (event: Event) => {
      closeBehavior = (event as CustomEvent<CloseBehavior>).detail;
    };

    window.addEventListener('close_behavior_changed', syncCloseBehavior);
    return () => window.removeEventListener('close_behavior_changed', syncCloseBehavior);
  });

  async function refreshConfig() {
    config = await getConfig();
    defaultMoveDestination = config.default_move_destination ?? '';
    const stale = Math.round(config.stale_threshold_seconds / 86400);
    const expiry = Math.round(config.default_ttl_seconds / 86400);
    const decayBufferDays = config.decaying_threshold_seconds / 86400;
    const decayStart = Math.max(stale + 1, Math.round(expiry - decayBufferDays));
    sliderValue = [
      Math.max(1, stale),
      Math.max(stale + 1, decayStart),
      Math.max(decayStart + 1, expiry),
    ];
    notificationsEnabled = config.notifications_enabled;
    startAtLogin = config.start_at_login;
    dropzoneEnabled = config.dropzone_enabled;
    closeBehavior = config.close_behavior;
  }

  async function addTargetWithPath(pathToAdd: string) {
    if (!config || !pathToAdd.trim()) return;

    const trimmedPath = pathToAdd.trim();
    const target = createWatchTarget(trimmedPath);

    await saveAddedTarget(target, config.watch_targets);
  }

  async function saveAddedTarget(target: WatchTarget, existingTargets: WatchTarget[]) {
    addingTarget = true;
    try {
      await updateWatchTargets([...existingTargets, target]);
      await refreshConfig();
      targetPath = '';
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorUpdateTargets')));
    } finally {
      addingTarget = false;
    }
  }

  async function savePreferences(): Promise<boolean> {
    if (!config) return false;

    showSavedIndicator = false;
    if (savedTimeoutId) {
      window.clearTimeout(savedTimeoutId);
      savedTimeoutId = null;
    }

    try {
      const trimmedDefaultMoveDestination = defaultMoveDestination.trim();
      await saveConfig({
        ...config,
        default_move_destination: trimmedDefaultMoveDestination || null,
        default_ttl_seconds: Math.max(1, sliderValue[2]) * 86400,
        stale_threshold_seconds: Math.max(1, sliderValue[0]) * 86400,
        decaying_threshold_seconds: Math.max(1, sliderValue[2] - sliderValue[1]) * 86400,
        notifications_enabled: notificationsEnabled,
        start_at_login: startAtLogin,
        dropzone_enabled: dropzoneEnabled,
        close_behavior: closeBehavior,
      });

      await refreshConfig();

      showSavedIndicator = true;
      savedTimeoutId = window.setTimeout(() => {
        showSavedIndicator = false;
      }, 2500);

      return true;
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorSavePrefs')));
      return false;
    }
  }

  async function replaceTarget(updated: WatchTarget): Promise<boolean> {
    if (!config) return false;

    try {
      await updateWatchTargets(
        config.watch_targets.map((target) => (target.id === updated.id ? updated : target)),
      );
      await refreshConfig();
      return true;
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorUpdateTarget')));
      return false;
    }
  }

  function initiateRemoveTarget(target: WatchTarget) {
    targetToRemove = target;
  }

  async function confirmRemoveTarget() {
    if (!config || !targetToRemove) return;
    const id = targetToRemove.id;
    targetToRemove = null;
    try {
      await updateWatchTargets(config.watch_targets.filter((t) => t.id !== id));
      await refreshConfig();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorRemoveTarget')));
    }
  }

  async function triggerManualScan() {
    try {
      await runReconciliationScan();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorReconcileScan')));
    }
  }

  function closeBehaviorLabel(value: CloseBehavior) {
    if (value === 'HideToTray') return i18n.t('settings.closeHideToTray');
    if (value === 'Quit') return i18n.t('settings.closeQuit');
    return i18n.t('settings.closeAsk');
  }
</script>

<PageHeader title={i18n.t('settings.title')} subtitle={i18n.t('settings.subtitle')} />

<PageBody>
  {#if config}
    <!-- Grid System for Forms (Full-width settings panel) -->
    <div class="grid grid-cols-1 gap-6">
      <!-- Form Settings & Watch Targets -->
      <div class="space-y-6">
        <!-- General Preferences Section -->
        <Card.Root>
          <Card.Content class="space-y-4">
            <div
              class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
            >
              <h3 class="text-sm font-semibold text-primary">
                {i18n.t('settings.general')}
              </h3>
              <div
                class="text-[11px] flex items-center gap-1.5 text-fluent-muted-light dark:text-fluent-muted-dark min-h-[1.5rem]"
              >
                {#if showSavedIndicator}
                  <span
                    class="text-green-600 dark:text-green-400 flex items-center gap-1 font-semibold transition-all duration-300"
                  >
                    <IconCheckmark class="w-3.5 h-3.5" />
                    {i18n.t('settings.savedShort')}
                  </span>
                {/if}
              </div>
            </div>

            <div
              class="grid grid-cols-1 md:grid-cols-2 gap-4 border-b border-fluent-border-light dark:border-fluent-border-dark pb-4"
            >
              <div class="flex flex-col gap-1.5">
                <Label for="default-move-destination">
                  {i18n.t('settings.defaultMoveDestination')}
                </Label>
                <InputGroup.Root>
                  <InputGroup.Input
                    id="default-move-destination"
                    bind:value={defaultMoveDestination}
                    placeholder={i18n.t('settings.defaultMoveDestinationPlaceholder')}
                    onchange={savePreferences}
                  />
                  <InputGroup.Addon align="inline-end">
                    <InputGroup.Button onclick={browseDefaultMoveDestination}>
                      {i18n.t('settings.browse')}
                    </InputGroup.Button>
                  </InputGroup.Addon>
                </InputGroup.Root>
              </div>

              <div class="flex flex-col gap-1.5">
                <Label for="lang-select">{i18n.t('lang.title')}</Label>
                <Select.Root
                  type="single"
                  value={i18n.currentLang}
                  onValueChange={(value) => i18n.setLang(value)}
                >
                  <Select.Trigger id="lang-select" class="w-full">
                    <span data-slot="select-value">
                      {i18n.t(`lang.${i18n.currentLang}`)}
                    </span>
                  </Select.Trigger>
                  <Select.Content>
                    <Select.Group>
                      {#each i18n.languages as language (language)}
                        <Select.Item value={language} label={i18n.t(`lang.${language}`)} />
                      {/each}
                    </Select.Group>
                  </Select.Content>
                </Select.Root>
              </div>

              <div class="flex flex-col gap-1.5">
                <Label for="theme-select">{i18n.t('theme.title')}</Label>
                <Select.Root
                  type="single"
                  value={i18n.currentTheme}
                  onValueChange={(value) => i18n.setTheme(value as 'light' | 'dark' | 'system')}
                >
                  <Select.Trigger id="theme-select" class="w-full">
                    <span data-slot="select-value">
                      {i18n.currentTheme === 'light'
                        ? i18n.t('theme.light')
                        : i18n.currentTheme === 'dark'
                          ? i18n.t('theme.dark')
                          : i18n.t('theme.system')}
                    </span>
                  </Select.Trigger>
                  <Select.Content>
                    <Select.Group>
                      <Select.Item value="light" label={i18n.t('theme.light')} />
                      <Select.Item value="dark" label={i18n.t('theme.dark')} />
                      <Select.Item value="system" label={i18n.t('theme.system')} />
                    </Select.Group>
                  </Select.Content>
                </Select.Root>
              </div>

              <div class="flex flex-col gap-1.5">
                <Label for="close-behavior-select">{i18n.t('settings.closeBehavior')}</Label>
                <Select.Root
                  type="single"
                  value={closeBehavior}
                  onValueChange={async (value) => {
                    closeBehavior = value as CloseBehavior;
                    await savePreferences();
                  }}
                >
                  <Select.Trigger id="close-behavior-select" class="w-full">
                    <span data-slot="select-value">{closeBehaviorLabel(closeBehavior)}</span>
                  </Select.Trigger>
                  <Select.Content>
                    <Select.Group>
                      <Select.Item value="Ask" label={i18n.t('settings.closeAsk')} />
                      <Select.Item value="HideToTray" label={i18n.t('settings.closeHideToTray')} />
                      <Select.Item value="Quit" label={i18n.t('settings.closeQuit')} />
                    </Select.Group>
                  </Select.Content>
                </Select.Root>
              </div>
            </div>

            <!-- Notification & Boot Toggles -->
            <Item.Group class="select-none flex flex-col gap-3">
              <Item.Root
                class="px-0 py-0 border-none hover:bg-transparent flex items-center justify-between"
              >
                <Item.Content class="flex flex-col gap-0.5">
                  <Item.Title
                    class="text-xs font-semibold text-neutral-800 dark:text-neutral-200 flex"
                  >
                    {i18n.t('settings.notifications')}
                  </Item.Title>
                </Item.Content>
                <Item.Actions class="flex-shrink-0 ml-4">
                  <Switch
                    checked={notificationsEnabled}
                    onCheckedChange={async () => {
                      notificationsEnabled = !notificationsEnabled;
                      await savePreferences();
                    }}
                    aria-label={i18n.t('settings.notifications')}
                  />
                </Item.Actions>
              </Item.Root>

              <Item.Root
                class="px-0 py-0 border-none hover:bg-transparent flex items-center justify-between"
              >
                <Item.Content class="flex flex-col gap-0.5">
                  <Item.Title
                    class="text-xs font-semibold text-neutral-800 dark:text-neutral-200 flex"
                  >
                    {i18n.t('settings.startAtLogin')}
                  </Item.Title>
                </Item.Content>
                <Item.Actions class="flex-shrink-0 ml-4">
                  <Switch
                    checked={startAtLogin}
                    onCheckedChange={async () => {
                      startAtLogin = !startAtLogin;
                      await savePreferences();
                    }}
                    aria-label={i18n.t('settings.startAtLogin')}
                  />
                </Item.Actions>
              </Item.Root>

              <Item.Root
                class="px-0 py-0 border-none hover:bg-transparent flex items-center justify-between"
              >
                <Item.Content class="flex flex-col gap-0.5">
                  <Item.Title class="text-xs font-semibold text-neutral-800 dark:text-neutral-200">
                    {i18n.t('settings.dropzone')}
                    <Tooltip.Root>
                      <Tooltip.Trigger>
                        <IconCircleHelp class="size-3.5" />
                      </Tooltip.Trigger>
                      <Tooltip.Content>
                        <p>{i18n.t('settings.dropzoneDesc')}</p>
                      </Tooltip.Content>
                    </Tooltip.Root>
                  </Item.Title>
                </Item.Content>
                <Item.Actions class="flex-shrink-0 ml-4">
                  <Switch
                    checked={dropzoneEnabled}
                    onCheckedChange={async () => {
                      dropzoneEnabled = !dropzoneEnabled;
                      await savePreferences();
                    }}
                    aria-label={i18n.t('settings.dropzone')}
                  />
                </Item.Actions>
              </Item.Root>
            </Item.Group>
          </Card.Content>
        </Card.Root>

        <!-- Decay Timeline Card -->
        <Card.Root>
          <Card.Content class="space-y-4">
            <div
              class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
            >
              <h3 class="text-sm font-semibold text-primary">
                {i18n.t('settings.decayTimeline')}
              </h3>
            </div>
            <p class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark leading-relaxed">
              {i18n.t('settings.decayTimelineDesc')}
            </p>
            <div class="pt-2">
              <DecayTimelineSlider bind:value={sliderValue} onCommit={savePreferences} />
            </div>
          </Card.Content>
        </Card.Root>

        <!-- Watch Targets Section -->
        <Card.Root>
          <Card.Content class="space-y-4">
            <div
              class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
            >
              <h3 class="text-sm font-semibold text-primary font-medium">
                {i18n.t('settings.watchTargets')}
              </h3>
              <Button
                type="button"
                variant="outline"
                onclick={triggerManualScan}
                disabled={filesState.syncing}
              >
                {#if filesState.syncing}
                  <Spinner data-icon="inline-start" />
                  <span>
                    {filesState.filesToProcess > 0
                      ? i18n.t('settings.reconcileProcessingProgress', {
                          current: filesState.filesProcessed.toLocaleString(),
                          total: filesState.filesToProcess.toLocaleString(),
                        })
                      : i18n.t('settings.reconcileScanning')}
                  </span>
                {:else}
                  <IconArrowSync data-icon="inline-start" />
                  <span>{i18n.t('settings.reconcileScan')}</span>
                {/if}
              </Button>
            </div>

            <!-- Add new folder form -->
            <div class="flex flex-col gap-1.5 w-full">
              <Label for="target-path">{i18n.t('settings.path')}</Label>
              <div class="flex flex-col md:flex-row gap-2">
                <InputGroup.Root class="flex-1">
                  <InputGroup.Input
                    id="target-path"
                    bind:value={targetPath}
                    placeholder={i18n.t('settings.path')}
                  />
                  <InputGroup.Addon align="inline-end">
                    <InputGroup.Button onclick={browseTargetFolder}>
                      {i18n.t('settings.browse')}
                    </InputGroup.Button>
                  </InputGroup.Addon>
                </InputGroup.Root>
                <Button
                  variant="outline"
                  onclick={() => addTargetWithPath(targetPath)}
                  disabled={addingTarget || !targetPath.trim()}
                >
                  {i18n.t('settings.addNewTarget')}
                </Button>
              </div>
            </div>

            <!-- Target list -->
            {#if config.watch_targets.length === 0}
              <div
                class="p-6 text-center text-xs text-fluent-muted-light dark:text-fluent-muted-dark border border-dashed border-fluent-border-light dark:border-fluent-border-dark rounded-md bg-neutral-50 dark:bg-neutral-900/40"
              >
                {i18n.t('settings.noTargets')}
              </div>
            {:else}
              <div class="space-y-3">
                {#each config.watch_targets as target (target.id)}
                  <WatchTargetCard
                    {target}
                    onUpdate={replaceTarget}
                    onRemove={initiateRemoveTarget}
                  />
                {/each}
              </div>
            {/if}
          </Card.Content>
        </Card.Root>
      </div>
    </div>
  {/if}
</PageBody>

<ConfirmDialog
  open={!!targetToRemove}
  title={i18n.t('settings.removeConfirmTitle')}
  message={targetToRemove
    ? `${i18n.t('settings.removeConfirmText')}\n\n${targetToRemove.path}`
    : ''}
  confirmLabel={i18n.t('settings.remove')}
  onCancel={() => (targetToRemove = null)}
  onConfirm={confirmRemoveTarget}
/>
