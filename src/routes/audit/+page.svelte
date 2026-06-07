<script lang="ts">
  import { onMount } from 'svelte';
  import AuditRow from '$lib/components/AuditRow.svelte';
  import { auditState } from '$lib/stores/audit.svelte';
  import { listen } from '@tauri-apps/api/event';
  import { i18n } from '$lib/i18n/i18n.svelte';

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

<div class="h-full flex flex-col min-h-0 relative">
  <!-- Header -->
  <header
    class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0"
  >
    <div>
      <h1 class="text-2xl font-bold tracking-tight">{i18n.t('audit.title')}</h1>
      <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
        {i18n.t('audit.subtitle')}
      </p>
    </div>
    <button class="fluent-button" onclick={() => auditState.refresh()}> Refresh </button>
  </header>

  <!-- Scrollable content -->
  <div class="flex-1 overflow-y-auto space-y-6 pt-4 pb-16 pr-1">
    <!-- Content -->
    {#if auditState.error}
      <div class="p-4 rounded bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300">
        {auditState.error}
      </div>
    {:else if auditState.loading && auditState.entries.length === 0}
      <div class="py-12 flex flex-col items-center justify-center gap-3">
        <svg class="animate-spin h-8 w-8 text-fluent-accent" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
          ></circle>
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
          ></path>
        </svg>
        <span class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark"
          >Loading audit entries...</span
        >
      </div>
    {:else if auditState.entries.length === 0}
      <div class="fluent-card py-16 text-center bg-fluent-card-light dark:bg-fluent-card-dark">
        <svg
          class="mx-auto h-12 w-12 text-fluent-muted-light dark:text-fluent-muted-dark opacity-50 mb-3"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="1.2"
            d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <h3 class="text-base font-semibold">{i18n.t('audit.noLogs')}</h3>
        <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
          Actions performed on files will be shown here, allowing you to review or revert them.
        </p>
      </div>
    {:else}
      <!-- Timeline Container -->
      <div
        class="relative pl-6 border-l-2 border-fluent-border-light dark:border-fluent-border-dark space-y-6 ml-3"
      >
        {#each auditState.entries as entry (entry.id)}
          <div class="relative">
            <!-- Timeline Node Circle -->
            <span
              class="absolute -left-[31px] top-1.5 flex items-center justify-center w-[12px] h-[12px] rounded-full bg-fluent-accent border-2 border-fluent-bg-light dark:border-fluent-bg-dark ring-4 ring-fluent-accent/15"
            ></span>

            <AuditRow {entry} onRefresh={() => auditState.refresh()} />
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
