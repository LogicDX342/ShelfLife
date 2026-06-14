<script lang="ts">
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule } from '$lib/types';
  import RuleCard from './RuleCard.svelte';
  import IconSpinner from '~icons/fluent/spinner-ios-20-regular';
  import IconDocumentBulletList from '~icons/fluent/document-bullet-list-24-regular';

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
    <IconSpinner class="animate-spin h-8 w-8 text-fluent-accent" />
    <span class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark">
      {i18n.t('rules.loading')}
    </span>
  </div>
{:else if rules.length === 0}
  <div class="fluent-card py-16 text-center">
    <IconDocumentBulletList
      class="mx-auto h-12 w-12 text-fluent-muted-light dark:text-fluent-muted-dark opacity-50 mb-3"
    />
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
