<script lang="ts">
  import { onMount } from 'svelte';
  import { filesState } from '$lib/stores/files.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import StatusBar from './StatusBar.svelte';
  import DashboardStatCard from './DashboardStatCard.svelte';

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
    <StatusBar files={filesState.files} />

    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
      <DashboardStatCard
        label={i18n.t('dashboard.triageNeeded')}
        value={filesState.counts.stale + filesState.counts.decaying}
        description={i18n.t('dashboard.triageDesc')}
        valueClass="text-amber-500"
      />
      <DashboardStatCard
        label={i18n.t('dashboard.healthyFiles')}
        value={filesState.counts.fresh + filesState.counts.pinned}
        description={i18n.t('dashboard.healthyDesc')}
        valueClass="text-green-500"
      />
      <DashboardStatCard
        label={i18n.t('dashboard.ignoredFiles')}
        value={filesState.counts.ignored}
        description={i18n.t('dashboard.ignoredDesc')}
        valueClass="text-neutral-500"
      />
    </div>
  </div>
</div>
