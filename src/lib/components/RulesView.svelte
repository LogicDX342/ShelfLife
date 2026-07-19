<script lang="ts">
  import { onMount } from 'svelte';

  import { deleteRule, saveRule } from '$lib/api/rules';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import { rulesState } from '$lib/stores/rules.svelte';
  import type { AutomationRule } from '$lib/types';
  import { getErrorMessage } from '$lib/utils/format';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import RuleEditor from './RuleEditor.svelte';
  import RuleList from './RuleList.svelte';

  let editingRule = $state<AutomationRule | null>(null);
  let ruleToDelete = $state<AutomationRule | null>(null);
  let showNewEditor = $state(false);

  let isEditing = $derived(showNewEditor || !!editingRule);

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

{#if isEditing}
  <PageHeader
    title={editingRule
      ? i18n.t('rules.editRule', { name: editingRule.name })
      : i18n.t('rules.newRule')}
  >
    {#snippet actions()}
      <Button variant="outline" onclick={handleCancel}>
        ← {i18n.t('rules.backToRules')}
      </Button>
    {/snippet}
  </PageHeader>

  <PageBody>
    <RuleEditor rule={editingRule} onSaved={refreshAfterSave} />
  </PageBody>
{:else}
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
    <RuleList
      rules={rulesState.rules}
      loading={rulesState.loading}
      onEdit={editRule}
      onDelete={initiateRemoveRule}
      onToggleEnabled={toggleRuleEnabled}
    />
  </PageBody>
{/if}

<ConfirmDialog
  open={!!ruleToDelete}
  title={i18n.t('rules.deleteConfirmTitle')}
  message={ruleToDelete ? `${i18n.t('rules.deleteConfirmText')}\n\n${ruleToDelete.name}` : ''}
  confirmLabel={i18n.t('rules.delete')}
  onCancel={() => (ruleToDelete = null)}
  onConfirm={confirmRemoveRule}
/>
