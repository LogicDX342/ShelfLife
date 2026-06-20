<script lang="ts">
  import IconDocumentBulletList from '@lucide/svelte/icons/list-checks';

  import EmptyState from '$lib/components/common/EmptyState.svelte';
  import LoadingState from '$lib/components/common/LoadingState.svelte';
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
  <LoadingState label={i18n.t('rules.loading')} />
{:else if rules.length === 0}
  <EmptyState
    icon={IconDocumentBulletList}
    title={i18n.t('rules.noRules')}
    description={i18n.t('rules.noRulesDesc')}
  />
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
