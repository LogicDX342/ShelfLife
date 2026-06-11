<script lang="ts">
  import { base } from '$app/paths';
  import { page } from '$app/stores';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { pauseWatching, resumeWatching } from '$lib/api/config';
  import { filesState } from '$lib/stores/files.svelte';

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
      icon: 'M4 5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5zm10 0a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1V5zM4 14a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-4zm10 0a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1v-4z',
    },
    {
      path: '/queue',
      labelKey: 'nav.queue',
      icon: 'M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01',
    },
    {
      path: '/browser',
      labelKey: 'nav.browser',
      icon: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z',
    },
    {
      path: '/rules',
      labelKey: 'nav.rules',
      icon: 'M3 5.5 L5.5 8 L9.5 4 M14 6 H21 M3 11.5 L5.5 14 L9.5 10 M14 12 H21 M3 17.5 L5.5 20 L9.5 16 M14 18 H21',
    },
    { path: '/audit', labelKey: 'nav.audit', icon: 'M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z' },
    {
      path: '/settings',
      labelKey: 'nav.settings',
      icon: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z',
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
    <svg
      class="w-6 h-6 text-fluent-accent flex-shrink-0"
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
      data-tauri-drag-region
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2.5"
        data-tauri-drag-region
        d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
      ></path>
    </svg>
    <span
      class="hidden md:inline text-lg font-bold tracking-wide whitespace-nowrap"
      data-tauri-drag-region>ShelfLife</span
    >
  </div>

  <!-- Navigation -->
  <nav class="flex-1 px-2 md:px-3 pb-4 space-y-1" aria-label="Primary">
    {#each navItems as item (item.path)}
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
        <svg
          class="w-5 h-5 flex-shrink-0 transition-transform duration-150 group-hover:scale-105"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d={item.icon}
          ></path>
        </svg>
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
          <svg class="w-4 h-4 text-fluent-accent animate-spin" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"
            ></circle>
            <path
              class="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
            ></path>
          </svg>
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
          <svg
            class="w-5 h-5 text-amber-500 dark:text-amber-400 transition-transform duration-150 group-hover:scale-105"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.8"
              d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"
            ></path>
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.8"
              d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            ></path>
          </svg>
          <span class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-amber-500"></span>
        {:else}
          <svg
            class="w-5 h-5 text-emerald-500 dark:text-emerald-400 transition-transform duration-150 group-hover:scale-105"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.8"
              d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z"
            ></path>
          </svg>
          <span class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-emerald-500 animate-pulse"
          ></span>
        {/if}
      </button>
    </div>
  </div>
</aside>
