<script lang="ts">
  import IconCheckmark from '@lucide/svelte/icons/check';
  import IconArrowSync from '@lucide/svelte/icons/refresh-cw';
  import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
  import { onMount } from 'svelte';

  import {
    getConfig,
    runReconciliationScan,
    saveConfig,
    updateWatchTargets,
  } from '$lib/api/config';
  import { selectDirectory } from '$lib/api/files';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import * as Item from '$lib/components/ui/item';
  import { Label } from '$lib/components/ui/label';
  import * as Select from '$lib/components/ui/select';
  import { Spinner } from '$lib/components/ui/spinner';
  import { Switch } from '$lib/components/ui/switch';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { filesState } from '$lib/stores/files.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type { AppConfig, CloseBehavior, WatchTarget } from '$lib/types';
  import { getErrorMessage } from '$lib/utils/format';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import DecayTimelineSlider from './DecayTimelineSlider.svelte';

  type PendingWatchTarget = {
    target: WatchTarget;
    overlappingTargets: WatchTarget[];
  };

  let config = $state<AppConfig | null>(null);
  let targetToRemove = $state<WatchTarget | null>(null);
  let pendingWatchTarget = $state<PendingWatchTarget | null>(null);
  let targetPath = $state('');
  let safeFolderPath = $state('');
  let sliderValue = $state([5, 29, 30]);
  let notificationsEnabled = $state(true);
  let startAtLogin = $state(false);
  let closeBehavior = $state<CloseBehavior>('Ask');
  let addingTarget = $state(false);
  let rejectedTargetId = $state<string | null>(null);
  let showSavedIndicator = $state(false);
  let savedTimeoutId: number | null = null;

  async function browseSafeFolder() {
    try {
      const selected = await selectDirectory('Select Safe Folder', safeFolderPath);
      if (selected) {
        safeFolderPath = selected;
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
        await addTargetWithPath(selected);
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
    safeFolderPath = config.safe_folder_path;
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
    closeBehavior = config.close_behavior;
  }

  async function addTarget() {
    await addTargetWithPath(targetPath);
  }

  async function addTargetWithPath(pathToAdd: string) {
    if (!config || !pathToAdd.trim()) return;

    const trimmedPath = pathToAdd.trim();

    // Prevent duplicate watch targets
    const isDuplicate = config.watch_targets.some(
      (target) => normalizeWatchPath(target.path) === normalizeWatchPath(trimmedPath),
    );
    if (isDuplicate) {
      notifications.error(i18n.t('settings.errorDuplicate'));
      return;
    }

    if (pathOverlapsSafeFolder(trimmedPath)) {
      notifications.error(i18n.t('settings.errorSafeFolderOverlap'));
      return;
    }

    const target = createWatchTarget(trimmedPath);
    const overlappingTargets = findOverlappingTargets(trimmedPath, config.watch_targets);
    if (overlappingTargets.length > 0) {
      pendingWatchTarget = { target, overlappingTargets };
      return;
    }

    await saveAddedTarget(target, config.watch_targets);
  }

  function createWatchTarget(path: string): WatchTarget {
    return {
      id: crypto.randomUUID(),
      path,
      enabled: true,
      recursive: false,
      default_ttl_seconds: null,
      ignore_patterns: [],
      include_hidden_patterns: [],
      rule_ids: [],
    };
  }

  function normalizeWatchPath(path: string): string {
    const normalized = path.trim().replaceAll('/', '\\').replace(/\\+$/, '');
    if (/^[a-z]:$/i.test(normalized)) {
      return `${normalized}\\`.toLowerCase();
    }
    return normalized.toLowerCase();
  }

  function pathContains(parent: string, child: string): boolean {
    const normalizedParent = normalizeWatchPath(parent);
    const normalizedChild = normalizeWatchPath(child);
    const parentPrefix = normalizedParent.endsWith('\\')
      ? normalizedParent
      : `${normalizedParent}\\`;

    return normalizedParent !== normalizedChild && normalizedChild.startsWith(parentPrefix);
  }

  function pathsOverlap(left: string, right: string): boolean {
    const normalizedLeft = normalizeWatchPath(left);
    const normalizedRight = normalizeWatchPath(right);
    return (
      normalizedLeft === normalizedRight || pathContains(left, right) || pathContains(right, left)
    );
  }

  function pathOverlapsSafeFolder(pathToAdd: string): boolean {
    const safePath = safeFolderPath.trim() || config?.safe_folder_path || '';
    return !!safePath && pathsOverlap(pathToAdd, safePath);
  }

  function safeFolderOverlapsEnabledTarget(safePath: string): boolean {
    return (
      !!safePath &&
      !!config?.watch_targets.some(
        (target) => target.enabled && pathsOverlap(safePath, target.path),
      )
    );
  }

  function findOverlappingTargets(pathToAdd: string, targets: WatchTarget[]): WatchTarget[] {
    return targets.filter(
      (target) => pathContains(pathToAdd, target.path) || pathContains(target.path, pathToAdd),
    );
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

  async function confirmOverlappingTarget() {
    if (!config || !pendingWatchTarget) return;

    const pending = pendingWatchTarget;
    pendingWatchTarget = null;
    const overlappingIds = new Set(pending.overlappingTargets.map((target) => target.id));
    await saveAddedTarget(
      pending.target,
      config.watch_targets.filter((target) => !overlappingIds.has(target.id)),
    );
  }

  function overlappingTargetMessage() {
    if (!pendingWatchTarget) return '';

    return i18n.t('settings.overlapConfirmText', {
      path: pendingWatchTarget.target.path,
      paths: pendingWatchTarget.overlappingTargets.map((target) => `- ${target.path}`).join('\n'),
    });
  }

  async function savePreferences(): Promise<boolean> {
    if (!config) return false;

    showSavedIndicator = false;
    if (savedTimeoutId) {
      window.clearTimeout(savedTimeoutId);
      savedTimeoutId = null;
    }

    try {
      const trimmedSafeFolderPath = safeFolderPath.trim();
      if (safeFolderOverlapsEnabledTarget(trimmedSafeFolderPath)) {
        notifications.error(i18n.t('settings.errorSafeFolderOverlap'));
        return false;
      }

      await saveConfig({
        ...config,
        safe_folder_path: trimmedSafeFolderPath,
        default_ttl_seconds: Math.max(1, sliderValue[2]) * 86400,
        stale_threshold_seconds: Math.max(1, sliderValue[0]) * 86400,
        decaying_threshold_seconds: Math.max(1, sliderValue[2] - sliderValue[1]) * 86400,
        notifications_enabled: notificationsEnabled,
        start_at_login: startAtLogin,
        close_behavior: closeBehavior,
      });

      const currentlyEnabled = await isEnabled();
      if (startAtLogin && !currentlyEnabled) {
        await enable();
      } else if (!startAtLogin && currentlyEnabled) {
        await disable();
      }

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
    if (updated.enabled && pathOverlapsSafeFolder(updated.path)) {
      notifications.error(i18n.t('settings.errorSafeFolderOverlap'));
      return false;
    }

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

  async function toggleTargetEnabled(target: WatchTarget, enabled: boolean) {
    if (enabled && pathOverlapsSafeFolder(target.path)) {
      notifications.error(i18n.t('settings.errorSafeFolderOverlap'));
      rejectedTargetId = target.id;
      window.setTimeout(() => {
        rejectedTargetId = null;
      }, 120);
      return;
    }

    await replaceTarget({ ...target, enabled });
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

<div class="h-full flex flex-col min-h-0 relative">
  <!-- Header -->
  <header
    class="border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0"
  >
    <h1 class="text-2xl font-bold tracking-tight">{i18n.t('settings.title')}</h1>
    <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
      {i18n.t('settings.subtitle')}
    </p>
  </header>

  <!-- Scrollable content -->
  <div class="flex-1 overflow-y-auto space-y-6 pt-4 pb-16 pr-1">
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
                  <Label for="safe-folder-path">{i18n.t('settings.safeFolder')}</Label>
                  <div class="flex gap-2">
                    <Input
                      id="safe-folder-path"
                      bind:value={safeFolderPath}
                      onchange={savePreferences}
                    />
                    <Button type="button" variant="outline" onclick={browseSafeFolder}>
                      {i18n.t('settings.browse')}
                    </Button>
                  </div>
                </div>

                <div class="flex flex-col gap-1.5">
                  <Label for="lang-select">{i18n.t('lang.title')}</Label>
                  <Select.Root
                    type="single"
                    value={i18n.currentLang}
                    onValueChange={(value) => i18n.setLang(value as 'en' | 'zh')}
                  >
                    <Select.Trigger id="lang-select" class="w-full">
                      <span data-slot="select-value">
                        {i18n.currentLang === 'zh' ? i18n.t('lang.zh') : i18n.t('lang.en')}
                      </span>
                    </Select.Trigger>
                    <Select.Content>
                      <Select.Item value="en" label={i18n.t('lang.en')} />
                      <Select.Item value="zh" label={i18n.t('lang.zh')} />
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
                      <Select.Item value="light" label={i18n.t('theme.light')} />
                      <Select.Item value="dark" label={i18n.t('theme.dark')} />
                      <Select.Item value="system" label={i18n.t('theme.system')} />
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
                      <Select.Item value="Ask" label={i18n.t('settings.closeAsk')} />
                      <Select.Item value="HideToTray" label={i18n.t('settings.closeHideToTray')} />
                      <Select.Item value="Quit" label={i18n.t('settings.closeQuit')} />
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
                      class="text-xs font-semibold text-neutral-800 dark:text-neutral-200"
                    >
                      {i18n.t('settings.notifications')}
                    </Item.Title>
                    <Item.Description
                      class="text-[11px] text-fluent-muted-light dark:text-fluent-muted-dark leading-normal line-clamp-none"
                    >
                      {i18n.t('settings.notificationsDesc')}
                    </Item.Description>
                  </Item.Content>
                  <Item.Actions class="flex-shrink-0 ml-4">
                    <Switch
                      checked={notificationsEnabled}
                      onclick={async () => {
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
                      class="text-xs font-semibold text-neutral-800 dark:text-neutral-200"
                    >
                      {i18n.t('settings.startAtLogin')}
                    </Item.Title>
                    <Item.Description
                      class="text-[11px] text-fluent-muted-light dark:text-fluent-muted-dark leading-normal line-clamp-none"
                    >
                      {i18n.t('settings.startAtLoginDesc')}
                    </Item.Description>
                  </Item.Content>
                  <Item.Actions class="flex-shrink-0 ml-4">
                    <Switch
                      checked={startAtLogin}
                      onclick={async () => {
                        startAtLogin = !startAtLogin;
                        await savePreferences();
                      }}
                      aria-label={i18n.t('settings.startAtLogin')}
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
              <p
                class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark leading-relaxed"
              >
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
                    <Spinner class="w-3.5 h-3.5 text-primary" />
                    <span>{i18n.t('settings.reconcileScanning')}</span>
                  {:else}
                    <IconArrowSync class="w-3.5 h-3.5 text-primary" />
                    <span>{i18n.t('settings.reconcileScan')}</span>
                  {/if}
                </Button>
              </div>

              <!-- Add new folder form -->
              <div class="flex flex-col gap-1.5 w-full">
                <Label for="target-path">{i18n.t('settings.path')}</Label>
                <div class="flex flex-col md:flex-row gap-2">
                  <div class="flex flex-1 gap-2">
                    <Input
                      id="target-path"
                      bind:value={targetPath}
                      placeholder={i18n.t('settings.path')}
                    />
                    <Button type="button" variant="outline" onclick={browseTargetFolder}>
                      {i18n.t('settings.browse')}
                    </Button>
                  </div>
                  <Button
                    variant="outline"
                    onclick={addTarget}
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
                    <div
                      class="p-3.5 bg-black/5 dark:bg-white/5 border border-fluent-border-light dark:border-fluent-border-dark rounded-md flex flex-col md:flex-row md:items-center justify-between gap-3 text-xs"
                    >
                      <div class="min-w-0 flex-1 space-y-1">
                        <p
                          class="font-semibold text-neutral-800 dark:text-neutral-200 truncate"
                          title={target.path}
                        >
                          {target.path}
                        </p>
                        <p
                          class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark flex items-center gap-2"
                        >
                          <span class="inline-flex items-center gap-1">
                            <span
                              class="w-1.5 h-1.5 rounded-full {target.enabled
                                ? 'bg-green-500'
                                : 'bg-neutral-400'}"
                            ></span>
                            {target.enabled
                              ? i18n.t('settings.enabled')
                              : i18n.t('settings.disabled')}
                          </span>
                          <span>•</span>
                          <span
                            >{target.recursive
                              ? i18n.t('settings.recursiveLabel')
                              : i18n.t('settings.topLevel')}</span
                          >
                        </p>
                      </div>

                      <!-- Switch & Button actions -->
                      <div class="flex items-center gap-3.5 flex-shrink-0">
                        <div
                          class={rejectedTargetId === target.id ? 'switch-rejected' : ''}
                          title="Toggle active status"
                        >
                          <Switch
                            checked={target.enabled}
                            onclick={() => toggleTargetEnabled(target, !target.enabled)}
                            aria-label="Toggle active status"
                          />
                        </div>

                        <Button
                          variant="outline"
                          onclick={() => replaceTarget({ ...target, recursive: !target.recursive })}
                        >
                          {target.recursive
                            ? i18n.t('settings.topLevel')
                            : i18n.t('settings.recursiveLabel')}
                        </Button>

                        <Button variant="destructive" onclick={() => initiateRemoveTarget(target)}>
                          {i18n.t('settings.remove')}
                        </Button>
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}
            </Card.Content>
          </Card.Root>
        </div>
      </div>
    {/if}
  </div>
</div>

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

<ConfirmDialog
  open={!!pendingWatchTarget}
  title={i18n.t('settings.overlapConfirmTitle')}
  message={overlappingTargetMessage()}
  confirmLabel={i18n.t('settings.overlapUseNew')}
  cancelLabel={i18n.t('settings.overlapKeepExisting')}
  onCancel={() => (pendingWatchTarget = null)}
  onConfirm={confirmOverlappingTarget}
/>
