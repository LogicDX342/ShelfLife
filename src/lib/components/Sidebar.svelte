<script lang="ts">
  import { base } from '$app/paths';
  import { page } from '$app/stores';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { pauseWatching, resumeWatching } from '$lib/api/config';

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
      path: '/rules',
      labelKey: 'nav.rules',
      icon: 'M10.3 21a2 2 0 0 1-1.4-.6l-5.3-5.3a2 2 0 0 1 0-2.8l9.9-9.9a2 2 0 0 1 2.8 0l5.3 5.3a2 2 0 0 1 0 2.8l-9.9 9.9a2 2 0 0 1-1.4.6zm3.5-17.1L4 13.8l5.3 5.3 9.9-9.9-5.4-5.3zM16 11a1 1 0 1 1 0-2 1 1 0 0 1 0 2z',
    },
    { path: '/audit', labelKey: 'nav.audit', icon: 'M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z' },
    {
      path: '/settings',
      labelKey: 'nav.settings',
      icon: 'M11.2 2.5c.3-.9 1.4-.9 1.7 0l.5 1.7c.1.4.5.7 1 .8l1.7.2c.9.1 1.2 1.2.6 1.8l-1.3 1.1c-.3.3-.5.7-.4 1.2l.4 1.7c.2.9-.8 1.6-1.5 1.1l-1.5-.9c-.4-.3-.9-.3-1.3 0l-1.5.9c-.8.5-1.8-.2-1.5-1.1l.4-1.7c.1-.4-.1-.9-.4-1.2L6.3 8.1c-.7-.6-.3-1.7.6-1.8l1.7-.2c.4-.1.8-.4.9-.8l.5-1.8zM12 15a3 3 0 100-6 3 3 0 000 6z',
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
  class="acrylic-sidebar w-full md:w-60 flex-shrink-0 flex flex-col border-b md:border-b-0 md:border-r border-fluent-border-light dark:border-fluent-border-dark select-none h-auto md:h-full overflow-y-auto"
>
  <!-- Brand / Header -->
  <div class="px-6 py-5 flex items-center gap-3">
    <svg
      class="w-6 h-6 text-fluent-accent"
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2.5"
        d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
      ></path>
    </svg>
    <span class="text-lg font-bold tracking-wide">ShelfLife</span>
  </div>

  <!-- Navigation -->
  <nav class="flex-1 px-3 py-4 space-y-1" aria-label="Primary">
    {#each navItems as item (item.path)}
      <a
        href="{base}{item.path}"
        class="relative flex items-center gap-3 px-4 py-2.5 rounded-md text-sm font-medium transition-all duration-150 group {isActive(
          item.path,
        )
          ? 'bg-black/5 dark:bg-white/5 font-semibold text-fluent-accent'
          : 'text-fluent-muted-light dark:text-fluent-muted-dark hover:bg-black/5 dark:hover:bg-white/5'}"
      >
        <!-- Active Nav Bar (Left accent) -->
        {#if isActive(item.path)}
          <div class="absolute left-0 top-2 bottom-2 w-1 rounded-full bg-fluent-accent"></div>
        {/if}
        <svg
          class="w-5 h-5 transition-transform duration-150 group-hover:scale-105"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d={item.icon}
          ></path>
        </svg>
        {i18n.t(item.labelKey)}
      </a>
    {/each}
  </nav>

  <!-- Controls band at the bottom -->
  <div class="p-4 border-t border-fluent-border-light dark:border-fluent-border-dark space-y-4">
    <!-- Watch Status Widget -->
    <div class="flex items-center justify-between p-2 rounded bg-black/5 dark:bg-white/5">
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
  </div>
</aside>
