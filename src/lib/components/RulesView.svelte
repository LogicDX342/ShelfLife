<script lang="ts">
  import { onMount } from 'svelte';

  import { deleteRule, saveRule, testRule } from '$lib/api/rules';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import { rulesState } from '$lib/stores/rules.svelte';
  import type { AutomationRule, RuleMatchExplanation } from '$lib/types';
  import { getErrorMessage } from '$lib/utils/format';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import RuleEditor from './RuleEditor.svelte';
  import RuleList from './RuleList.svelte';
  import RuleTestResults from './RuleTestResults.svelte';

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

<PageHeader title={i18n.t('rules.title')} subtitle={i18n.t('rules.subtitle')}>
  {#snippet actions()}
    <Button
      onclick={() => {
        showNewEditor = true;
        editingRule = null;
      }}
    >
      + {i18n.t('rules.newRule')}
    </Button>
  {/snippet}
</PageHeader>

<PageBody>
  {#if showNewEditor || editingRule}
    <div class="space-y-4">
      <h2 class="text-lg font-semibold text-primary">
        {editingRule
          ? i18n.t('rules.editRule', { name: editingRule.name })
          : i18n.t('rules.newRule')}
      </h2>
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
</PageBody>

<ConfirmDialog
  open={!!ruleToDelete}
  title={i18n.t('rules.deleteConfirmTitle')}
  message={ruleToDelete ? `${i18n.t('rules.deleteConfirmText')}\n\n${ruleToDelete.name}` : ''}
  confirmLabel={i18n.t('rules.delete')}
  onCancel={() => (ruleToDelete = null)}
  onConfirm={confirmRemoveRule}
/>
