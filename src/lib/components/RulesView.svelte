<script lang="ts">
  import { onMount } from 'svelte';

  import { getConfig } from '$lib/api/config';
  import { deleteRule, saveRule } from '$lib/api/rules';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { createRuleFromTemplate, type RuleTemplate } from '$lib/rules/templates';
  import { notifications } from '$lib/stores/notifications.svelte';
  import { rulesState } from '$lib/stores/rules.svelte';
  import type { AppConfig, AutomationRule } from '$lib/types';
  import { getErrorMessage } from '$lib/utils/format';

  import ConfirmDialog from './ConfirmDialog.svelte';
  import RuleEditor from './RuleEditor.svelte';
  import RuleList from './RuleList.svelte';
  import RuleTemplatePicker from './RuleTemplatePicker.svelte';

  let config = $state<AppConfig | null>(null);
  let editingRule = $state<AutomationRule | null>(null);
  let templateDraft = $state<AutomationRule | null>(null);
  let ruleToDelete = $state<AutomationRule | null>(null);
  let initialLoad = $state(true);
  let showNewEditor = $state(false);
  let showTemplates = $state(false);

  let isEditing = $derived(showNewEditor || !!editingRule || !!templateDraft);
  let isChoosingTemplate = $derived(
    showTemplates || (!initialLoad && !rulesState.loading && rulesState.rules.length === 0),
  );

  onMount(() => {
    void loadInitialState();
  });

  async function loadInitialState() {
    initialLoad = true;
    [config] = await Promise.all([getConfig().catch(() => null), rulesState.refresh()]);
    initialLoad = false;
  }

  function initiateRemoveRule(rule: AutomationRule) {
    ruleToDelete = rule;
  }

  function editRule(rule: AutomationRule) {
    editingRule = rule;
    templateDraft = null;
    showNewEditor = false;
  }

  function createNewRule() {
    editingRule = null;
    templateDraft = null;
    showNewEditor = true;
  }

  function useTemplate(template: RuleTemplate, localizedName: string) {
    editingRule = null;
    templateDraft = createRuleFromTemplate(template, config, localizedName);
    showNewEditor = true;
    showTemplates = false;
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
    templateDraft = null;
    showNewEditor = false;
    showTemplates = false;
    await rulesState.refresh();
  }

  function handleCancel() {
    editingRule = null;
    templateDraft = null;
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
    <RuleEditor rule={editingRule} draft={templateDraft} onSaved={refreshAfterSave} />
  </PageBody>
{:else}
  <PageHeader title={i18n.t('rules.title')} subtitle={i18n.t('rules.subtitle')}>
    {#snippet actions()}
      {#if isChoosingTemplate && rulesState.rules.length > 0}
        <Button variant="outline" onclick={() => (showTemplates = false)}>
          ← {i18n.t('rules.backToRules')}
        </Button>
      {:else if !isChoosingTemplate}
        <Button variant="outline" onclick={() => (showTemplates = true)}>
          {i18n.t('rules.useTemplate')}
        </Button>
      {/if}
      <Button onclick={createNewRule}>
        + {i18n.t('rules.newRule')}
      </Button>
    {/snippet}
  </PageHeader>

  <PageBody>
    {#if isChoosingTemplate}
      <RuleTemplatePicker {config} onSelect={useTemplate} />
    {:else}
      <RuleList
        rules={rulesState.rules}
        loading={initialLoad || rulesState.loading}
        onEdit={editRule}
        onDelete={initiateRemoveRule}
        onToggleEnabled={toggleRuleEnabled}
      />
    {/if}
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
