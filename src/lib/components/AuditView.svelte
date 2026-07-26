<script lang="ts">
  import IconHistory from '@lucide/svelte/icons/history';
  import IconSearch from '@lucide/svelte/icons/search';

  import AuditRow from '$lib/components/AuditRow.svelte';
  import EmptyState from '$lib/components/common/EmptyState.svelte';
  import LoadingState from '$lib/components/common/LoadingState.svelte';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import * as Alert from '$lib/components/ui/alert';
  import { Button } from '$lib/components/ui/button';
  import * as InputGroup from '$lib/components/ui/input-group';
  import { Spinner } from '$lib/components/ui/spinner';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { auditState } from '$lib/stores/audit.svelte';

  let searchInputValue = $state('');
  let debounceTimer: ReturnType<typeof setTimeout>;

  $effect(() => {
    const query = searchInputValue;
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      void auditState.setSearchQuery(query);
    }, 200);
    return () => clearTimeout(debounceTimer);
  });
</script>

<!-- Header -->
<PageHeader title={i18n.t('audit.title')} subtitle={i18n.t('audit.subtitle')}>
  {#snippet actions()}
    <InputGroup.Root>
      <InputGroup.Input
        type="text"
        placeholder={i18n.t('audit.search')}
        bind:value={searchInputValue}
      />
      <InputGroup.Addon align="inline-end">
        {i18n.t('audit.results', { count: auditState.totalCount })}
      </InputGroup.Addon>
      <InputGroup.Addon>
        <IconSearch />
      </InputGroup.Addon>
    </InputGroup.Root>
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
  {:else if auditState.totalCount === 0 && auditState.searchQuery}
    <EmptyState
      icon={IconHistory}
      title={i18n.t('audit.noSearchResults')}
      description={i18n.t('audit.noSearchResultsDesc')}
    />
  {:else if auditState.totalCount === 0}
    <EmptyState
      icon={IconHistory}
      title={i18n.t('audit.noLogs')}
      description={i18n.t('audit.noLogsDesc')}
    />
  {:else}
    <div class="flex flex-col gap-6">
      <!-- Timeline Container -->
      <div class="relative ml-3 flex flex-col gap-6 border-l-2 pl-6">
        {#each auditState.entries as entry (entry.id)}
          <div class="relative">
            <!-- Timeline Node Circle -->
            <span
              class="absolute -left-[31px] top-1.5 size-3 rounded-full bg-primary border-2 border-background ring-4 ring-primary/15"
            ></span>

            <AuditRow {entry} />
          </div>
        {/each}
      </div>

      {#if auditState.hasMore}
        <div class="flex justify-center">
          <Button
            type="button"
            variant="outline"
            disabled={auditState.loadingMore}
            onclick={() => void auditState.loadMore()}
          >
            {#if auditState.loadingMore}
              <Spinner data-icon="inline-start" aria-label={i18n.t('audit.loadingMore')} />
              {i18n.t('audit.loadingMore')}
            {:else}
              {i18n.t('audit.loadMore', {
                count: auditState.totalCount - auditState.entries.length,
              })}
            {/if}
          </Button>
        </div>
      {/if}
    </div>
  {/if}
</PageBody>
