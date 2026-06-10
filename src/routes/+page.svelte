<script lang="ts">
  import { onMount } from 'svelte';
  import { filesState } from '$lib/stores/files.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import StatusBar from '$lib/components/StatusBar.svelte';

  onMount(() => {
    filesState.refresh();
  });
</script>

<div class="h-full flex flex-col gap-6">
  <header
    class="border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0"
  >
    <h1 class="text-2xl font-bold tracking-tight">{i18n.t('nav.dashboard')}</h1>
    <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
      {i18n.t('dashboard.welcome')}
    </p>
  </header>

  <div class="space-y-6">
    <!-- Statistics Widget -->
    <StatusBar files={filesState.files} />

    <!-- Simple summary cards for stats -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
      <div
        class="fluent-card p-6 flex flex-col justify-between h-32 bg-fluent-card-light dark:bg-fluent-card-dark"
      >
        <span
          class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark uppercase tracking-wider"
          >{i18n.t('dashboard.triageNeeded')}</span
        >
        <span class="text-3xl font-extrabold text-amber-500 mt-2">
          {filesState.counts.stale + filesState.counts.decaying}
        </span>
        <p class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
          {i18n.t('dashboard.triageDesc')}
        </p>
      </div>

      <div
        class="fluent-card p-6 flex flex-col justify-between h-32 bg-fluent-card-light dark:bg-fluent-card-dark"
      >
        <span
          class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark uppercase tracking-wider"
          >{i18n.t('dashboard.healthyFiles')}</span
        >
        <span class="text-3xl font-extrabold text-green-500 mt-2">
          {filesState.counts.fresh + filesState.counts.pinned}
        </span>
        <p class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
          {i18n.t('dashboard.healthyDesc')}
        </p>
      </div>

      <div
        class="fluent-card p-6 flex flex-col justify-between h-32 bg-fluent-card-light dark:bg-fluent-card-dark"
      >
        <span
          class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark uppercase tracking-wider"
          >{i18n.t('dashboard.ignoredFiles')}</span
        >
        <span class="text-3xl font-extrabold text-neutral-500 mt-2">
          {filesState.counts.ignored}
        </span>
        <p class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
          {i18n.t('dashboard.ignoredDesc')}
        </p>
      </div>
    </div>
  </div>
</div>
