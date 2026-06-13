<script lang="ts">
  import { onMount } from 'svelte';
  import { getErrorMessage } from '$lib/utils/format';
  import RuleEditor from './RuleEditor.svelte';
  import RuleList from './RuleList.svelte';
  import RuleTestResults from './RuleTestResults.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { deleteRule, testRule, saveRule } from '$lib/api/rules';
  import { rulesState } from '$lib/stores/rules.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule, RuleMatchExplanation } from '$lib/types';
  import { notifications } from '$lib/stores/notifications.svelte';

  let previewResults = $state<RuleMatchExplanation[]>([]);
  let editingRule = $state<AutomationRule | null>(null);
  let ruleToDelete = $state<AutomationRule | null>(null);
  let showNewEditor = $state(false);
  let selectedPreviewRuleName = $state<string | null>(null);
  let testingRuleId = $state<string | null>(null);

  onMount(() => {
    rulesState.refresh();
  });

  function initiateRemoveRule(rule: AutomationRule) {
    ruleToDelete = rule;
  }

  function editRule(rule: AutomationRule) {
    editingRule = rule;
    showNewEditor = false;
  }

  async function confirmRemoveRule() {
    if (!ruleToDelete) return;
    const id = ruleToDelete.id;
    ruleToDelete = null;
    try {
      await deleteRule(id);
      if (editingRule?.id === id) {
        editingRule = null;
      }
      await rulesState.refresh();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorDelete')));
    }
  }

  async function toggleRuleEnabled(rule: AutomationRule) {
    try {
      const updated = { ...rule, enabled: !rule.enabled };
      await saveRule(updated);
      await rulesState.refresh();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorUpdateStatus')));
    }
  }

  async function previewRule(rule: AutomationRule) {
    testingRuleId = rule.id;
    selectedPreviewRuleName = rule.name;
    try {
      previewResults = await testRule(rule);
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorTest')));
    } finally {
      testingRuleId = null;
    }
  }

  async function refreshAfterSave() {
    editingRule = null;
    showNewEditor = false;
    await rulesState.refresh();
  }

  function handleCancel() {
    editingRule = null;
    showNewEditor = false;
  }
</script>

<div class="h-full flex flex-col min-h-0 relative">
  <header
    class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0"
  >
    <div>
      <h1 class="text-2xl font-bold tracking-tight">{i18n.t('rules.title')}</h1>
      <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
        {i18n.t('rules.subtitle')}
      </p>
    </div>
    <div class="flex items-center gap-2">
      <button class="fluent-button" onclick={() => rulesState.refresh()}>
        {i18n.t('rules.refresh')}
      </button>
      <button
        class="fluent-button fluent-button-primary"
        onclick={() => {
          showNewEditor = true;
          editingRule = null;
        }}
      >
        + {i18n.t('rules.newRule')}
      </button>
    </div>
  </header>

  <div class="flex-1 overflow-y-auto space-y-6 pt-4 pb-16 pr-1">
    {#if showNewEditor || editingRule}
      <div
        class="fluent-card p-6 bg-fluent-card-light dark:bg-fluent-card-dark border border-fluent-accent/20 shadow-md"
      >
        <h3 class="text-base font-semibold mb-4 text-fluent-accent">
          {editingRule
            ? i18n.t('rules.editRule', { name: editingRule.name })
            : i18n.t('rules.newRule')}
        </h3>
        <RuleEditor rule={editingRule} onSaved={refreshAfterSave} onCancel={handleCancel} />
      </div>
    {/if}

    <RuleList
      rules={rulesState.rules}
      loading={rulesState.loading}
      {testingRuleId}
      onEdit={editRule}
      onTest={previewRule}
      onDelete={initiateRemoveRule}
      onToggleEnabled={toggleRuleEnabled}
    />

    <RuleTestResults
      results={previewResults}
      ruleName={selectedPreviewRuleName}
      onClear={() => (previewResults = [])}
    />
  </div>
</div>

<ConfirmDialog
  open={!!ruleToDelete}
  title={i18n.t('rules.deleteConfirmTitle')}
  message={ruleToDelete ? `${i18n.t('rules.deleteConfirmText')}\n\n${ruleToDelete.name}` : ''}
  confirmLabel={i18n.t('rules.delete')}
  onCancel={() => (ruleToDelete = null)}
  onConfirm={confirmRemoveRule}
/>
