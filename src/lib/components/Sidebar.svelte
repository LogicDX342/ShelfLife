<script lang="ts">
  import IconPauseCircle from '@lucide/svelte/icons/circle-pause';
  import IconPlayCircle from '@lucide/svelte/icons/circle-play';
  import IconFolderOpen from '@lucide/svelte/icons/folder-open';
  import IconHistory from '@lucide/svelte/icons/history';
  import IconInfo from '@lucide/svelte/icons/info';
  import IconBoard from '@lucide/svelte/icons/layout-dashboard';
  import IconClipboardList from '@lucide/svelte/icons/list-todo';
  import IconSettings from '@lucide/svelte/icons/settings';
  import IconFlash from '@lucide/svelte/icons/zap';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';

  import { assets, resolve } from '$app/paths';
  import { page } from '$app/state';
  import { isWatchingPaused, pauseWatching, resumeWatching } from '$lib/api/config';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Separator } from '$lib/components/ui/separator';
  import { Spinner } from '$lib/components/ui/spinner';
  import { Switch } from '$lib/components/ui/switch';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { filesState } from '$lib/stores/files.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import { getErrorMessage } from '$lib/utils/format';

  type AppRoute = '/' | '/queue' | '/browser' | '/rules' | '/audit' | '/settings' | '/about';
  type NavItem = {
    path: AppRoute;
    labelKey: string;
    icon: typeof IconBoard;
  };

  let isPaused = $state(false);
  let watchStatusBusy = $state(true);

  onMount(() => {
    let active = true;
    let receivedStateEvent = false;

    const reportError = (reason: unknown) => {
      if (active) {
        notifications.error(getErrorMessage(reason, i18n.t('status.watchError')));
      }
    };
    const unlistenTask = listen<boolean>('watching_paused_changed', ({ payload }) => {
      if (!active) return;
      receivedStateEvent = true;
      isPaused = payload;
    }).catch((reason) => {
      reportError(reason);
      return () => {};
    });

    void unlistenTask
      .then(() => isWatchingPaused())
      .then((paused) => {
        if (active && !receivedStateEvent) {
          isPaused = paused;
        }
      })
      .catch(reportError)
      .finally(() => {
        if (active) {
          watchStatusBusy = false;
        }
      });

    return () => {
      active = false;
      void unlistenTask.then((unlisten) => unlisten());
    };
  });

  async function setWatchingEnabled(enabled: boolean) {
    if (watchStatusBusy) return;

    watchStatusBusy = true;
    try {
      if (enabled) {
        await resumeWatching();
      } else {
        await pauseWatching();
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('status.watchError')));
    } finally {
      watchStatusBusy = false;
    }
  }

  const navItems: NavItem[] = [
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
  ];

  const secondaryNavItems: NavItem[] = [
    { path: '/settings', labelKey: 'nav.settings', icon: IconSettings },
    { path: '/about', labelKey: 'nav.about', icon: IconInfo },
  ];

  function isActive(path: AppRoute) {
    if (path === '/') {
      return page.url.pathname === '/';
    }
    return page.url.pathname.startsWith(path);
  }
</script>

<aside
  class="w-16 md:w-60 flex-shrink-0 flex flex-col border-r bg-sidebar/95 text-sidebar-foreground select-none h-full overflow-hidden backdrop-blur-xl"
>
  <!-- Brand / Header -->
  <div
    class="px-3 md:px-6 pt-5 pb-2 flex items-center justify-center md:justify-start gap-3"
    data-tauri-drag-region
  >
    <img
      src={`${assets}/favicon.svg`}
      alt="ShelfLife"
      class="size-8 flex-shrink-0 object-contain"
      draggable="false"
      data-tauri-drag-region
    />
    <span
      class="hidden md:inline text-lg font-bold tracking-wide whitespace-nowrap"
      data-tauri-drag-region>ShelfLife</span
    >
  </div>

  <!-- Navigation -->
  <nav
    class="flex-1 px-2 md:px-3 pb-4 space-y-1 overflow-y-auto overflow-x-hidden"
    aria-label="Primary"
  >
    {#each navItems as item (item.path)}
      {@const Icon = item.icon}
      <Button
        href={resolve(item.path)}
        variant="ghost"
        class="relative w-full justify-center md:justify-start gap-3 px-3 md:px-4 py-2.5 text-sm font-medium group {isActive(
          item.path,
        )
          ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold'
          : 'text-muted-foreground hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground'}"
        title={i18n.t(item.labelKey)}
      >
        {#if isActive(item.path)}
          <div class="absolute left-0 top-2 bottom-2 w-1 rounded-full bg-primary"></div>
        {/if}
        <Icon
          class="w-5 h-5 flex-shrink-0 transition-transform duration-150 group-hover:scale-105"
        />
        <span class="hidden md:inline whitespace-nowrap">{i18n.t(item.labelKey)}</span>
      </Button>
    {/each}
  </nav>

  <!-- Controls band at the bottom -->
  <div class="p-2 md:p-4">
    <nav class="mb-3 flex flex-col gap-1" aria-label="Secondary">
      {#each secondaryNavItems as item (item.path)}
        {@const Icon = item.icon}
        <Button
          href={resolve(item.path)}
          variant="ghost"
          class="relative w-full justify-center md:justify-start gap-3 px-3 md:px-4 py-2.5 text-sm font-medium group {isActive(
            item.path,
          )
            ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold'
            : 'text-muted-foreground hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground'}"
          title={i18n.t(item.labelKey)}
        >
          {#if isActive(item.path)}
            <div class="absolute left-0 top-2 bottom-2 w-1 rounded-full bg-primary"></div>
          {/if}
          <Icon
            class="w-5 h-5 flex-shrink-0 transition-transform duration-150 group-hover:scale-105"
          />
          <span class="hidden md:inline whitespace-nowrap">{i18n.t(item.labelKey)}</span>
        </Button>
      {/each}
    </nav>

    <Separator class="mb-4" />
    <!-- Watch Status Widget - Expanded -->
    <div class="hidden md:block">
      <Card.Root>
        <Card.Content class="flex items-center gap-3">
          <div class="flex min-w-0 flex-1 flex-col">
            <span class="text-xs text-muted-foreground">{i18n.t('status.watchStatus')}</span>
            {#if filesState.syncing}
              <span class="flex items-center gap-1.5 text-sm font-medium">
                <Spinner class="size-3.5 text-primary" />
                {i18n.t('status.syncing')}
              </span>
            {:else}
              <span class="text-sm font-medium">
                {isPaused ? i18n.t('status.paused') : i18n.t('status.active')}
              </span>
            {/if}
          </div>
          <Switch
            checked={!isPaused}
            onCheckedChange={setWatchingEnabled}
            disabled={watchStatusBusy}
            aria-label={i18n.t('status.watchStatus')}
            class="flex-shrink-0"
          />
        </Card.Content>
      </Card.Root>
    </div>

    <!-- Watch Status Button - Shrunk -->
    <div class="flex md:hidden justify-center">
      <Button
        onclick={() => setWatchingEnabled(isPaused)}
        disabled={watchStatusBusy}
        variant="ghost"
        class="relative"
        title={`${i18n.t('status.watchStatus')}: ${isPaused ? i18n.t('status.paused') : i18n.t('status.active')}`}
      >
        {#if watchStatusBusy}
          <Spinner />
        {:else if isPaused}
          <IconPlayCircle
            class="w-5 h-5 text-amber-500 dark:text-amber-400 transition-transform duration-150 group-hover:scale-105"
          />
          <span class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-amber-500"></span>
        {:else}
          <IconPauseCircle
            class="w-5 h-5 text-emerald-500 dark:text-emerald-400 transition-transform duration-150 group-hover:scale-105"
          />
          {#if filesState.syncing}
            <Spinner class="absolute -right-0.5 -top-0.5 size-3.5 text-primary" />
          {:else}
            <span
              class="absolute right-1.5 top-1.5 size-2 rounded-full bg-emerald-500 animate-pulse"
            ></span>
          {/if}
        {/if}
      </Button>
    </div>
  </div>
</aside>
