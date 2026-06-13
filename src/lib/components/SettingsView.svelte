<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getConfig,
    saveConfig,
    updateWatchTargets,
    runReconciliationScan,
  } from '$lib/api/config';
  import { selectDirectory } from '$lib/api/files';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { getErrorMessage } from '$lib/utils/format';
  import type { AppConfig, WatchTarget } from '$lib/types';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { filesState } from '$lib/stores/files.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';

  type PendingWatchTarget = {
    target: WatchTarget;
    overlappingTargets: WatchTarget[];
  };

  let config = $state<AppConfig | null>(null);
  let targetToRemove = $state<WatchTarget | null>(null);
  let pendingWatchTarget = $state<PendingWatchTarget | null>(null);
  let targetPath = $state('');
  let safeFolderPath = $state('');
  let defaultTtlDays = $state(30);
  let staleThresholdDays = $state(5);
  let decayingThresholdHours = $state(24);
  let notificationsEnabled = $state(true);
  let savingPrefs = $state(false);
  let addingTarget = $state(false);
  let rejectedTargetId = $state<string | null>(null);

  async function browseSafeFolder() {
    try {
      const selected = await selectDirectory('Select Safe Folder', safeFolderPath);
      if (selected) {
        safeFolderPath = selected;
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

  onMount(async () => {
    await refreshConfig();
  });

  async function refreshConfig() {
    config = await getConfig();
    safeFolderPath = config.safe_folder_path;
    defaultTtlDays = Math.round(config.default_ttl_seconds / 86400);
    staleThresholdDays = Math.round(config.stale_threshold_seconds / 86400);
    decayingThresholdHours = Math.round(config.decaying_threshold_seconds / 3600);
    notificationsEnabled = config.notifications_enabled;
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

  async function savePreferences() {
    if (!config) return;
    savingPrefs = true;
    try {
      const trimmedSafeFolderPath = safeFolderPath.trim();
      if (safeFolderOverlapsEnabledTarget(trimmedSafeFolderPath)) {
        notifications.error(i18n.t('settings.errorSafeFolderOverlap'));
        return;
      }

      await saveConfig({
        ...config,
        safe_folder_path: trimmedSafeFolderPath,
        default_ttl_seconds: Math.max(1, defaultTtlDays) * 86400,
        stale_threshold_seconds: Math.max(1, staleThresholdDays) * 86400,
        decaying_threshold_seconds: Math.max(1, decayingThresholdHours) * 3600,
        notifications_enabled: notificationsEnabled,
      });
      await refreshConfig();
      notifications.success(i18n.t('settings.saved'));
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('settings.errorSavePrefs')));
    } finally {
      savingPrefs = false;
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

  async function toggleTargetEnabled(target: WatchTarget, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.checked && pathOverlapsSafeFolder(target.path)) {
      notifications.error(i18n.t('settings.errorSafeFolderOverlap'));
      rejectedTargetId = target.id;
      window.setTimeout(() => {
        input.checked = false;
        rejectedTargetId = null;
      }, 120);
      return;
    }

    const saved = await replaceTarget({ ...target, enabled: input.checked });
    if (!saved) {
      input.checked = target.enabled;
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
          <section class="fluent-card p-6 space-y-4">
            <h3
              class="text-sm font-semibold text-fluent-accent border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
            >
              {i18n.t('settings.general')}
            </h3>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <label class="flex flex-col gap-1.5">
                <span
                  class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                  >{i18n.t('settings.safeFolder')}</span
                >
                <div class="flex gap-2">
                  <input bind:value={safeFolderPath} class="fluent-input text-xs flex-1 min-w-0" />
                  <button
                    type="button"
                    class="fluent-button text-xs font-semibold px-3 flex-shrink-0"
                    onclick={browseSafeFolder}
                  >
                    {i18n.t('settings.browse')}
                  </button>
                </div>
              </label>

              <label class="flex flex-col gap-1.5">
                <span
                  class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                  >{i18n.t('settings.defaultTtlDays')}</span
                >
                <input
                  min="1"
                  type="number"
                  bind:value={defaultTtlDays}
                  class="fluent-input text-xs"
                />
              </label>

              <label class="flex flex-col gap-1.5">
                <span
                  class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                  >{i18n.t('settings.staleAge')}</span
                >
                <input
                  min="1"
                  type="number"
                  bind:value={staleThresholdDays}
                  class="fluent-input text-xs"
                />
              </label>

              <label class="flex flex-col gap-1.5">
                <span
                  class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                  >{i18n.t('settings.decayBuffer')}</span
                >
                <input
                  min="1"
                  type="number"
                  bind:value={decayingThresholdHours}
                  class="fluent-input text-xs"
                />
              </label>

              <label class="flex flex-col gap-1.5">
                <span
                  class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                  >{i18n.t('lang.title')}</span
                >
                <select
                  id="lang-select"
                  class="fluent-input text-xs"
                  value={i18n.currentLang}
                  onchange={(e) =>
                    i18n.setLang((e.target as HTMLSelectElement).value as 'en' | 'zh')}
                >
                  <option value="en">{i18n.t('lang.en')}</option>
                  <option value="zh">{i18n.t('lang.zh')}</option>
                </select>
              </label>

              <label class="flex flex-col gap-1.5">
                <span
                  class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                  >{i18n.t('theme.title')}</span
                >
                <select
                  id="theme-select"
                  class="fluent-input text-xs"
                  value={i18n.currentTheme}
                  onchange={(e) =>
                    i18n.setTheme(
                      (e.target as HTMLSelectElement).value as 'light' | 'dark' | 'system',
                    )}
                >
                  <option value="light">{i18n.t('theme.light')}</option>
                  <option value="dark">{i18n.t('theme.dark')}</option>
                  <option value="system">{i18n.t('theme.system')}</option>
                </select>
              </label>
            </div>

            <!-- Notification Toggles -->
            <div class="flex items-center gap-3 pt-2 select-none">
              <span
                class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                >{i18n.t('settings.notifications')}</span
              >
              <label class="fluent-switch">
                <input
                  type="checkbox"
                  class="fluent-switch-input"
                  checked={notificationsEnabled}
                  onchange={() => (notificationsEnabled = !notificationsEnabled)}
                />
                <span class="fluent-switch-track">
                  <span class="fluent-switch-thumb"></span>
                </span>
              </label>
            </div>

            <!-- Action buttons -->
            <div class="pt-2">
              <button
                class="fluent-button fluent-button-primary text-xs font-semibold"
                onclick={savePreferences}
                disabled={savingPrefs}
              >
                {savingPrefs ? i18n.t('settings.saving') : i18n.t('settings.save')}
              </button>
            </div>
          </section>

          <!-- Watch Targets Section -->
          <section class="fluent-card p-6 space-y-4">
            <div
              class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
            >
              <h3 class="text-sm font-semibold text-fluent-accent font-medium">
                {i18n.t('settings.watchTargets')}
              </h3>
              <button
                type="button"
                class="fluent-button text-[11px] font-semibold py-1 px-2.5 flex items-center gap-1.5"
                onclick={triggerManualScan}
                disabled={filesState.syncing}
              >
                {#if filesState.syncing}
                  <svg
                    class="w-3.5 h-3.5 text-fluent-accent animate-spin"
                    fill="none"
                    viewBox="0 0 24 24"
                  >
                    <circle
                      class="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      stroke-width="3"
                    ></circle>
                    <path
                      class="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    ></path>
                  </svg>
                  <span>{i18n.t('settings.reconcileScanning')}</span>
                {:else}
                  <svg
                    class="w-3.5 h-3.5 text-fluent-accent"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                    xmlns="http://www.w3.org/2000/svg"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99"
                    ></path>
                  </svg>
                  <span>{i18n.t('settings.reconcileScan')}</span>
                {/if}
              </button>
            </div>

            <!-- Add new folder form -->
            <div class="flex flex-col md:flex-row gap-2">
              <div class="flex flex-1 gap-2">
                <input
                  bind:value={targetPath}
                  placeholder={i18n.t('settings.path')}
                  class="fluent-input flex-1 text-xs min-w-0"
                />
                <button
                  type="button"
                  class="fluent-button text-xs font-semibold px-3 flex-shrink-0"
                  onclick={browseTargetFolder}
                >
                  {i18n.t('settings.browse')}
                </button>
              </div>
              <button
                class="fluent-button text-xs font-semibold md:flex-shrink-0"
                onclick={addTarget}
                disabled={addingTarget || !targetPath.trim()}
              >
                {i18n.t('settings.addNewTarget')}
              </button>
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
                      <label
                        class="fluent-switch {rejectedTargetId === target.id
                          ? 'fluent-switch-rejected'
                          : ''}"
                        title="Toggle active status"
                      >
                        <input
                          type="checkbox"
                          class="fluent-switch-input"
                          checked={target.enabled}
                          onchange={(event) => toggleTargetEnabled(target, event)}
                        />
                        <span class="fluent-switch-track">
                          <span class="fluent-switch-thumb"></span>
                        </span>
                      </label>

                      <button
                        class="fluent-button p-1.5 text-[10px] font-semibold"
                        onclick={() => replaceTarget({ ...target, recursive: !target.recursive })}
                      >
                        {target.recursive
                          ? i18n.t('settings.topLevel')
                          : i18n.t('settings.recursiveLabel')}
                      </button>

                      <button
                        class="fluent-button p-1.5 text-[10px] font-semibold text-red-600 dark:text-red-400"
                        onclick={() => initiateRemoveTarget(target)}
                      >
                        {i18n.t('settings.remove')}
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </section>
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
