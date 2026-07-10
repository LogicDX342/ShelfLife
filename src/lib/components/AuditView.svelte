<script lang="ts">
  import IconHistory from '@lucide/svelte/icons/history';

  import AuditRow from '$lib/components/AuditRow.svelte';
  import EmptyState from '$lib/components/common/EmptyState.svelte';
  import LoadingState from '$lib/components/common/LoadingState.svelte';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import * as Alert from '$lib/components/ui/alert';
  import { Button } from '$lib/components/ui/button';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { auditState } from '$lib/stores/audit.svelte';
</script>

<!-- Header -->
<PageHeader title={i18n.t('audit.title')} subtitle={i18n.t('audit.subtitle')}>
  {#snippet actions()}
    <Button variant="outline" onclick={() => auditState.refresh()}>
      {i18n.t('rules.refresh')}
    </Button>
  {/snippet}
</PageHeader>

<PageBody>
  <!-- Content -->
  {#if auditState.error}
    <Alert.Root variant="destructive">
      <Alert.Description>{auditState.error}</Alert.Description>
    </Alert.Root>
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
    <div class="relative ml-3 flex flex-col gap-6 border-l-2 pl-6">
      {#each auditState.entries as entry (entry.id)}
        <div class="relative">
          <!-- Timeline Node Circle -->
          <span
            class="absolute -left-[31px] top-1.5 w-3 h-3 rounded-full bg-fluent-accent border-2 border-fluent-bg-light dark:border-fluent-bg-dark ring-4 ring-fluent-accent/15"
          ></span>

          <AuditRow {entry} />
        </div>
      {/each}
    </div>
  {/if}
</PageBody>
