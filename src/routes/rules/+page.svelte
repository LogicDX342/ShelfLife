<script lang="ts">
  import { onMount } from 'svelte';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';
  import RuleEditor from '$lib/components/RuleEditor.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { deleteRule, testRule, saveRule } from '$lib/api/rules';
  import { rulesState } from '$lib/stores/rules.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule, RuleMatchExplanation } from '$lib/types';

  let previewResults = $state<RuleMatchExplanation[]>([]);
  let editingRule = $state<AutomationRule | null>(null);
  let ruleToDelete = $state<AutomationRule | null>(null);
  let showNewEditor = $state(false);
  let error = $state<string | null>(null);
  let selectedPreviewRuleName = $state<string | null>(null);
  let testingRuleId = $state<string | null>(null);

  onMount(() => {
    rulesState.refresh();
  });

  function initiateRemoveRule(rule: AutomationRule) {
    ruleToDelete = rule;
  }

  async function confirmRemoveRule() {
    if (!ruleToDelete) return;
    const id = ruleToDelete.id;
    ruleToDelete = null;
    error = null;
    try {
      await deleteRule(id);
      if (editingRule?.id === id) {
        editingRule = null;
      }
      await rulesState.refresh();
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not delete rule.');
    }
  }

  async function toggleRuleEnabled(rule: AutomationRule) {
    error = null;
    try {
      const updated = { ...rule, enabled: !rule.enabled };
      await saveRule(updated);
      await rulesState.refresh();
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not update rule status.');
    }
  }

  async function previewRule(rule: AutomationRule) {
    error = null;
    testingRuleId = rule.id;
    selectedPreviewRuleName = rule.name;
    try {
      previewResults = await testRule(rule);
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not test rule.');
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
  <!-- Header -->
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
      <button class="fluent-button" onclick={() => rulesState.refresh()}> Refresh </button>
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

  <!-- Scrollable content -->
  <div class="flex-1 overflow-y-auto space-y-6 pt-4 pb-16 pr-1">
    <!-- Rule Creator/Editor View -->
    {#if showNewEditor || editingRule}
      <div
        class="fluent-card p-6 bg-fluent-card-light dark:bg-fluent-card-dark border border-fluent-accent/20 shadow-md"
      >
        <h3 class="text-base font-semibold mb-4 text-fluent-accent">
          {editingRule ? `Edit Rule: ${editingRule.name}` : i18n.t('rules.newRule')}
        </h3>
        <RuleEditor rule={editingRule} onSaved={refreshAfterSave} onCancel={handleCancel} />
      </div>
    {/if}

    {#if error}
      <div
        class="p-3 text-sm rounded bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-900/50"
      >
        {error}
      </div>
    {/if}

    <!-- Rules List -->
    {#if rulesState.loading && rulesState.rules.length === 0}
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
          >Loading automation rules...</span
        >
      </div>
    {:else if rulesState.rules.length === 0}
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
          Create a rule to automate trashing, moving, or renaming files in your watch targets.
        </p>
      </div>
    {:else}
      <section class="space-y-3">
        {#each rulesState.rules as rule (rule.id)}
          <div
            class="fluent-card flex flex-col md:flex-row md:items-center justify-between gap-4 p-4 hover:border-fluent-accent/30"
          >
            <!-- Rule details -->
            <div class="space-y-1 min-w-0 flex-1">
              <div class="flex items-center gap-2.5">
                <span
                  class="font-semibold text-sm tracking-tight text-fluent-text-light dark:text-fluent-text-dark"
                  >{rule.name}</span
                >
                <span
                  class="text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded bg-black/5 dark:bg-white/5 text-fluent-muted-light dark:text-fluent-muted-dark"
                >
                  Priority: {rule.priority}
                </span>
              </div>

              <p
                class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark truncate"
                title={rule.watch_path}
              >
                Watch target: {rule.watch_path}
              </p>

              <!-- Conditions pill row -->
              <div
                class="flex flex-wrap items-center gap-1.5 pt-1 text-[10px] font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
              >
                <span
                  class="px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-950/40 text-blue-700 dark:text-blue-300"
                >
                  Mode: {rule.mode}
                </span>
                {#if typeof rule.action === 'string'}
                  <span
                    class="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300"
                  >
                    Action: {rule.action}
                  </span>
                {:else if 'Move' in rule.action}
                  <span
                    class="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300"
                  >
                    Move to: {rule.action.Move.destination_path.split('/').pop() ||
                      rule.action.Move.destination_path}
                  </span>
                {:else if 'Rename' in rule.action}
                  <span
                    class="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300"
                  >
                    Rename template: {rule.action.Rename.template}
                  </span>
                {/if}
                {#if rule.action !== 'Ignore'}
                  <span
                    class="px-1.5 py-0.5 rounded bg-neutral-100 dark:bg-neutral-800 text-neutral-600 dark:text-neutral-400"
                  >
                    TTL: {Math.round(rule.ttl_seconds / 86400)} days
                  </span>
                {/if}
              </div>
            </div>

            <!-- Toggle + Buttons -->
            <div class="flex items-center gap-3.5 flex-shrink-0">
              <!-- Active Toggle switch -->
              <div class="flex items-center gap-2">
                <span class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark">
                  {rule.enabled ? i18n.t('rules.enabled') : i18n.t('rules.disabled')}
                </span>
                <label class="fluent-switch">
                  <input
                    type="checkbox"
                    class="fluent-switch-input"
                    checked={rule.enabled}
                    onchange={() => toggleRuleEnabled(rule)}
                  />
                  <span class="fluent-switch-track">
                    <span class="fluent-switch-thumb"></span>
                  </span>
                </label>
              </div>

              <!-- Action buttons -->
              <div class="flex items-center gap-1.5">
                <button
                  class="fluent-button p-1.5 text-xs font-semibold"
                  onclick={() => {
                    editingRule = rule;
                    showNewEditor = false;
                  }}
                  aria-label="Edit Rule"
                >
                  Edit
                </button>
                <button
                  class="fluent-button p-1.5 text-xs font-semibold"
                  onclick={() => previewRule(rule)}
                  disabled={testingRuleId === rule.id}
                  aria-label="Test Rule"
                >
                  {#if testingRuleId === rule.id}
                    Testing...
                  {:else}
                    {i18n.t('rules.testRule')}
                  {/if}
                </button>
                <button
                  class="fluent-button p-1.5 text-xs font-semibold text-red-600 dark:text-red-400"
                  onclick={() => initiateRemoveRule(rule)}
                  aria-label="Delete Rule"
                >
                  Delete
                </button>
              </div>
            </div>
          </div>
        {/each}
      </section>
    {/if}

    <!-- Test Results Panel -->
    {#if previewResults.length > 0}
      <section
        class="fluent-card p-6 bg-fluent-card-light dark:bg-fluent-card-dark border border-fluent-accent/20 space-y-4"
      >
        <div
          class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
        >
          <h3
            class="text-sm font-semibold tracking-tight text-fluent-text-light dark:text-fluent-text-dark"
          >
            {i18n.t('rules.testResults')}:
            <span class="text-fluent-accent">{selectedPreviewRuleName}</span>
          </h3>
          <button
            class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark hover:underline"
            onclick={() => (previewResults = [])}
          >
            Clear Results
          </button>
        </div>

        <div class="flex flex-col gap-2 max-h-72 overflow-y-auto pr-1">
          {#each previewResults as result (result.file_path)}
            <div
              class="p-3 bg-black/5 dark:bg-white/5 border border-fluent-border-light dark:border-fluent-border-dark rounded-md flex flex-col md:flex-row md:items-center justify-between gap-3 text-xs"
            >
              <div class="space-y-0.5 min-w-0 flex-1">
                <p class="font-semibold text-neutral-800 dark:text-neutral-200 truncate">
                  {result.file_path.split('/').pop() || result.file_path}
                </p>
                <p class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark truncate">
                  {result.file_path}
                </p>
              </div>
              {#if result.size_bytes !== null}
                <span
                  class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark flex-shrink-0"
                >
                  {formatBytes(result.size_bytes)}
                </span>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}
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
