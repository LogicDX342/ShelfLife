<script lang="ts">
  import { base } from '$app/paths';
  import { page } from '$app/stores';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { pauseWatching, resumeWatching } from '$lib/api/config';
  import { filesState } from '$lib/stores/files.svelte';
  import IconBoard from '~icons/fluent/board-24-regular';
  import IconClipboardList from '~icons/fluent/clipboard-task-list-ltr-24-regular';
  import IconFolderOpen from '~icons/fluent/folder-open-24-regular';
  import IconFlash from '~icons/fluent/flash-24-regular';
  import IconHistory from '~icons/fluent/history-24-regular';
  import IconSettings from '~icons/fluent/settings-24-regular';
  import IconArchive from '~icons/fluent/archive-24-regular';
  import IconPlayCircle from '~icons/fluent/play-circle-24-regular';
  import IconPauseCircle from '~icons/fluent/pause-circle-24-regular';
  import IconSpinner from '~icons/fluent/spinner-ios-20-regular';

  let isPaused = $state(false);

  async function toggleWatchStatus() {
    try {
      if (isPaused) {
        await resumeWatching();
        isPaused = false;
      } else {
        await pauseWatching();
        isPaused = true;
      }
    } catch (e) {
      console.error(e);
    }
  }

  const navItems = [
    {
      path: '/',
      labelKey: 'nav.dashboard',
      icon: IconBoard,
    },
    {
      path: '/queue',
      labelKey: 'nav.queue',
      icon: IconClipboardList,
    },
    {
      path: '/browser',
      labelKey: 'nav.browser',
      icon: IconFolderOpen,
    },
    {
      path: '/rules',
      labelKey: 'nav.rules',
      icon: IconFlash,
    },
    { path: '/audit', labelKey: 'nav.audit', icon: IconHistory },
    {
      path: '/settings',
      labelKey: 'nav.settings',
      icon: IconSettings,
    },
  ];

  function isActive(path: string) {
    if (path === '/') {
      return $page.url.pathname === '/';
    }
    return $page.url.pathname.startsWith(path);
  }
</script>

<aside
  class="acrylic-sidebar w-16 md:w-60 flex-shrink-0 flex flex-col border-r border-fluent-border-light dark:border-fluent-border-dark select-none h-full overflow-y-auto overflow-x-hidden"
>
  <!-- Brand / Header -->
  <div
    class="px-3 md:px-6 pt-5 pb-2 flex items-center justify-center md:justify-start gap-3"
    data-tauri-drag-region
  >
    <IconArchive class="w-6 h-6 text-fluent-accent flex-shrink-0" data-tauri-drag-region />
    <span
      class="hidden md:inline text-lg font-bold tracking-wide whitespace-nowrap"
      data-tauri-drag-region>ShelfLife</span
    >
  </div>

  <!-- Navigation -->
  <nav class="flex-1 px-2 md:px-3 pb-4 space-y-1" aria-label="Primary">
    {#each navItems as item (item.path)}
      {@const Icon = item.icon}
      <a
        href="{base}{item.path}"
        class="relative flex items-center justify-center md:justify-start gap-3 px-3 md:px-4 py-2.5 rounded-md text-sm font-medium transition-all duration-150 group {isActive(
          item.path,
        )
          ? 'bg-black/5 dark:bg-white/5 font-semibold text-fluent-accent'
          : 'text-fluent-muted-light dark:text-fluent-muted-dark hover:bg-black/5 dark:hover:bg-white/5'}"
        title={i18n.t(item.labelKey)}
      >
        <!-- Active Nav Bar (Left accent) -->
        {#if isActive(item.path)}
          <div class="absolute left-0 top-2 bottom-2 w-1 rounded-full bg-fluent-accent"></div>
        {/if}
        <Icon
          class="w-5 h-5 flex-shrink-0 transition-transform duration-150 group-hover:scale-105"
        />
        <span class="hidden md:inline whitespace-nowrap">{i18n.t(item.labelKey)}</span>
      </a>
    {/each}
  </nav>

  <!-- Controls band at the bottom -->
  <div class="p-2 md:p-4 border-t border-fluent-border-light dark:border-fluent-border-dark">
    <!-- Global Sync Indicator (Active only during scans lasting >= 1s) -->
    {#if filesState.syncing && filesState.syncDuration >= 1}
      <!-- Expanded Sidebar -->
      <div
        class="hidden md:flex items-center gap-3 p-2.5 rounded bg-blue-500/10 dark:bg-blue-400/5 border border-blue-500/15 mb-3 transition-all duration-300"
      >
        <div class="relative flex items-center justify-center flex-shrink-0 w-5 h-5">
          <!-- Outer spinning ring -->
          <div
            class="absolute inset-0 rounded-full border-2 border-fluent-accent/20 border-t-fluent-accent animate-spin"
          ></div>
          <!-- Inner pulsing active indicator dot -->
          <div class="w-1.5 h-1.5 rounded-full bg-fluent-accent animate-pulse"></div>
        </div>
        <div class="flex flex-col min-w-0 w-full gap-0.5">
          <span class="text-[11px] font-semibold text-fluent-text-light dark:text-fluent-text-dark"
            >Syncing files...</span
          >
          <span class="text-[9px] text-fluent-muted-light dark:text-fluent-muted-dark truncate">
            {filesState.filesScanned.toLocaleString()} files ({Math.floor(
              filesState.syncDuration,
            )}s)
          </span>
          {#if filesState.currentPath}
            <span
              class="text-[8px] text-fluent-muted-light/70 dark:text-fluent-muted-dark/70 truncate"
              title={filesState.currentPath}
            >
              {filesState.currentPath}
            </span>
          {/if}
        </div>
      </div>

      <!-- Collapsed Sidebar -->
      <div
        class="flex md:hidden justify-center mb-3 relative group"
        title="Syncing {filesState.filesScanned} files ({Math.floor(filesState.syncDuration)}s)..."
      >
        <div
          class="w-10 h-10 flex items-center justify-center rounded-md bg-blue-500/10 dark:bg-blue-400/5 border border-blue-500/15"
        >
          <IconSpinner class="w-4 h-4 text-fluent-accent animate-spin" />
          <span
            class="absolute -top-1 -right-1 bg-fluent-accent text-white text-[8px] font-bold px-1.5 py-0.5 rounded-full scale-75 shadow-sm"
          >
            {filesState.filesScanned > 999 ? '999+' : filesState.filesScanned}
          </span>
        </div>
      </div>
    {/if}

    <!-- Watch Status Widget - Expanded -->
    <div class="hidden md:flex items-center justify-between p-2 rounded bg-black/5 dark:bg-white/5">
      <div class="flex flex-col">
        <span class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('status.watchStatus')}</span
        >
        <span class="text-sm font-medium"
          >{isPaused ? i18n.t('status.paused') : i18n.t('status.active')}</span
        >
      </div>
      <label class="fluent-switch">
        <input
          type="checkbox"
          class="fluent-switch-input"
          checked={!isPaused}
          onchange={toggleWatchStatus}
        />
        <span class="fluent-switch-track">
          <span class="fluent-switch-thumb"></span>
        </span>
      </label>
    </div>

    <!-- Watch Status Button - Shrunk -->
    <div class="flex md:hidden justify-center">
      <button
        onclick={toggleWatchStatus}
        class="w-10 h-10 flex items-center justify-center rounded-md transition-all duration-150 text-fluent-muted-light dark:text-fluent-muted-dark hover:bg-black/5 dark:hover:bg-white/5 relative group focus:outline-none"
        title="{i18n.t('status.watchStatus')}: {isPaused
          ? i18n.t('status.paused')
          : i18n.t('status.active')}"
      >
        {#if isPaused}
          <IconPlayCircle
            class="w-5 h-5 text-amber-500 dark:text-amber-400 transition-transform duration-150 group-hover:scale-105"
          />
          <span class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-amber-500"></span>
        {:else}
          <IconPauseCircle
            class="w-5 h-5 text-emerald-500 dark:text-emerald-400 transition-transform duration-150 group-hover:scale-105"
          />
          <span class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-emerald-500 animate-pulse"
          ></span>
        {/if}
      </button>
    </div>
  </div>
</aside>
