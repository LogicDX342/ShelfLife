<script lang="ts">
  import { onMount } from 'svelte';
  import { getConfig, saveConfig, updateWatchTargets } from '$lib/api/config';
  import { selectDirectory } from '$lib/api/files';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { getErrorMessage } from '$lib/utils/format';
  import type { AppConfig, WatchTarget } from '$lib/types';

  let config = $state<AppConfig | null>(null);
  let targetPath = $state('');
  let safeFolderPath = $state('');
  let defaultTtlDays = $state(30);
  let staleThresholdDays = $state(5);
  let decayingThresholdHours = $state(24);
  let notificationsEnabled = $state(true);
  let error = $state<string | null>(null);
  let successMessage = $state<string | null>(null);
  let savingPrefs = $state(false);
  let addingTarget = $state(false);

  async function browseSafeFolder() {
    try {
      const selected = await selectDirectory('Select Safe Folder', safeFolderPath);
      if (selected) {
        safeFolderPath = selected;
      }
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not select folder.');
    }
  }

  async function browseTargetFolder() {
    try {
      const selected = await selectDirectory('Select Watch Target Folder', targetPath);
      if (selected) {
        targetPath = selected;
      }
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not select folder.');
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
    if (!config || !targetPath.trim()) return;
    addingTarget = true;
    error = null;
    const targets = [
      ...config.watch_targets,
      {
        id: crypto.randomUUID(),
        path: targetPath.trim(),
        enabled: true,
        recursive: false,
        default_ttl_seconds: null,
        ignore_patterns: [],
        include_hidden_patterns: [],
        rule_ids: [],
      },
    ];
    try {
      await updateWatchTargets(targets);
      await refreshConfig();
      targetPath = '';
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not update watch targets.');
    } finally {
      addingTarget = false;
    }
  }

  async function savePreferences() {
    if (!config) return;
    error = null;
    successMessage = null;
    savingPrefs = true;
    try {
      await saveConfig({
        ...config,
        safe_folder_path: safeFolderPath.trim(),
        default_ttl_seconds: Math.max(1, defaultTtlDays) * 86400,
        stale_threshold_seconds: Math.max(1, staleThresholdDays) * 86400,
        decaying_threshold_seconds: Math.max(1, decayingThresholdHours) * 3600,
        notifications_enabled: notificationsEnabled,
      });
      await refreshConfig();
      successMessage = i18n.t('settings.saved');
      setTimeout(() => (successMessage = null), 4000);
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not save preferences.');
    } finally {
      savingPrefs = false;
    }
  }

  async function replaceTarget(updated: WatchTarget) {
    if (!config) return;
    try {
      await updateWatchTargets(
        config.watch_targets.map((target) => (target.id === updated.id ? updated : target)),
      );
      await refreshConfig();
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not update watch target.');
    }
  }

  async function removeTarget(id: string) {
    if (!config) return;
    if (!confirm('Are you sure you want to remove this watch target?')) return;
    try {
      await updateWatchTargets(config.watch_targets.filter((target) => target.id !== id));
      await refreshConfig();
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not remove watch target.');
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
                    Browse...
                  </button>
                </div>
              </label>

              <label class="flex flex-col gap-1.5">
                <span
                  class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark"
                  >Default Expiry (TTL Days)</span
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
                  >Stale Age (Days)</span
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
                  >Decay Buffer (Hours)</span
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

            <!-- Status Alerts -->
            {#if successMessage}
              <div
                class="p-2.5 text-xs rounded bg-green-100 dark:bg-green-950/40 text-green-700 dark:text-green-300"
              >
                {successMessage}
              </div>
            {/if}

            <!-- Action buttons -->
            <div class="pt-2">
              <button
                class="fluent-button fluent-button-primary text-xs font-semibold"
                onclick={savePreferences}
                disabled={savingPrefs}
              >
                {savingPrefs ? 'Saving Preferences...' : i18n.t('settings.save')}
              </button>
            </div>
          </section>

          <!-- Watch Targets Section -->
          <section class="fluent-card p-6 space-y-4">
            <h3
              class="text-sm font-semibold text-fluent-accent border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
            >
              {i18n.t('settings.watchTargets')}
            </h3>

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
                  Browse...
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
                          {target.enabled ? 'Enabled' : 'Disabled'}
                        </span>
                        <span>•</span>
                        <span>{target.recursive ? 'Recursive' : 'Top Level'}</span>
                      </p>
                    </div>

                    <!-- Switch & Button actions -->
                    <div class="flex items-center gap-3.5 flex-shrink-0">
                      <label class="fluent-switch" title="Toggle active status">
                        <input
                          type="checkbox"
                          class="fluent-switch-input"
                          checked={target.enabled}
                          onchange={() => replaceTarget({ ...target, enabled: !target.enabled })}
                        />
                        <span class="fluent-switch-track">
                          <span class="fluent-switch-thumb"></span>
                        </span>
                      </label>

                      <button
                        class="fluent-button p-1.5 text-[10px] font-semibold"
                        onclick={() => replaceTarget({ ...target, recursive: !target.recursive })}
                      >
                        {target.recursive ? 'Top level' : 'Recursive'}
                      </button>

                      <button
                        class="fluent-button p-1.5 text-[10px] font-semibold text-red-600 dark:text-red-400"
                        onclick={() => removeTarget(target.id)}
                      >
                        Remove
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

    {#if error}
      <div
        class="p-3 text-xs rounded bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-900/50"
      >
        {error}
      </div>
    {/if}
  </div>
</div>
