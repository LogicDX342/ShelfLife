<script lang="ts">
  import { onMount } from 'svelte';

  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { filesState } from '$lib/stores/files.svelte';

  import DashboardStatCard from './DashboardStatCard.svelte';
  import StatusBar from './StatusBar.svelte';

  onMount(() => {
    filesState.refresh();
  });
</script>

<PageHeader title={i18n.t('nav.dashboard')} subtitle={i18n.t('dashboard.welcome')} />

<div class="space-y-6 mt-6">
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
