<script lang="ts">
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule } from '$lib/types';
  import RuleCard from './RuleCard.svelte';

  let {
    rules,
    loading = false,
    testingRuleId = null,
    onEdit,
    onTest,
    onDelete,
    onToggleEnabled,
  } = $props<{
    rules: AutomationRule[];
    loading?: boolean;
    testingRuleId?: string | null;
    onEdit: (rule: AutomationRule) => void;
    onTest: (rule: AutomationRule) => void;
    onDelete: (rule: AutomationRule) => void;
    onToggleEnabled: (rule: AutomationRule) => void;
  }>();
</script>

{#if loading && rules.length === 0}
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
    <span class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark">
      {i18n.t('rules.loading')}
    </span>
  </div>
{:else if rules.length === 0}
  <div class="fluent-card py-16 text-center">
    <svg
      class="mx-auto h-12 w-12 text-fluent-muted-light dark:text-fluent-muted-dark opacity-50 mb-3"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="1"
        d="M10.3 21a2 2 0 0 1-1.4-.6l-5.3-5.3a2 2 0 0 1 0-2.8l9.9-9.9a2 2 0 0 1 2.8 0l5.3 5.3a2 2 0 0 1 0 2.8l-9.9 9.9a2 2 0 0 1-1.4.6z"
      />
    </svg>
    <h3 class="text-base font-semibold">{i18n.t('rules.noRules')}</h3>
    <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
      {i18n.t('rules.noRulesDesc')}
    </p>
  </div>
{:else}
  <section class="space-y-3">
    {#each rules as rule (rule.id)}
      <RuleCard
        {rule}
        testing={testingRuleId === rule.id}
        {onEdit}
        {onTest}
        {onDelete}
        {onToggleEnabled}
      />
    {/each}
  </section>
{/if}
