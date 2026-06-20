<script lang="ts">
  import IconHistory from '@lucide/svelte/icons/history';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';

  import AuditRow from '$lib/components/AuditRow.svelte';
  import EmptyState from '$lib/components/common/EmptyState.svelte';
  import LoadingState from '$lib/components/common/LoadingState.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { auditState } from '$lib/stores/audit.svelte';

  onMount(() => {
    auditState.refresh();

    let active = true;
    let unlistenAudit: (() => void) | null = null;

    listen('audit_updated', () => auditState.refresh()).then((unlisten) => {
      if (active) unlistenAudit = unlisten;
      else unlisten();
    });

    return () => {
      active = false;
      unlistenAudit?.();
    };
  });
</script>

<!-- Header -->
<PageHeader title={i18n.t('audit.title')} subtitle={i18n.t('audit.subtitle')}>
  {#snippet actions()}
    <Button variant="outline" onclick={() => auditState.refresh()}>
      {i18n.t('rules.refresh')}
    </Button>
  {/snippet}
</PageHeader>

<!-- Scrollable content -->
<div class="flex-1 overflow-y-auto space-y-6 pt-4 pb-16 pr-1">
  <!-- Content -->
  {#if auditState.error}
    <div class="p-4 rounded bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300">
      {auditState.error}
    </div>
  {:else if auditState.loading && auditState.entries.length === 0}
    <LoadingState label={i18n.t('audit.loading')} />
  {:else if auditState.entries.length === 0}
    <EmptyState
      icon={IconHistory}
      title={i18n.t('audit.noLogs')}
      description={i18n.t('audit.noLogsDesc')}
    />
  {:else}
    <!-- Timeline Container -->
    <div
      class="relative pl-6 border-l-2 border-fluent-border-light dark:border-fluent-border-dark space-y-6 ml-3"
    >
      {#each auditState.entries as entry (entry.id)}
        <div class="relative">
          <!-- Timeline Node Circle -->
          <span
            class="absolute -left-[31px] top-1.5 w-3 h-3 rounded-full bg-fluent-accent border-2 border-fluent-bg-light dark:border-fluent-bg-dark ring-4 ring-fluent-accent/15"
          ></span>

          <AuditRow {entry} onRefresh={() => auditState.refresh()} />
        </div>
      {/each}
    </div>
  {/if}
</div>
