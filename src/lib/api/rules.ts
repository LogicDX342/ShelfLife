import { invoke } from '@tauri-apps/api/core';
import type { AutomationRule, RuleMatchExplanation } from '$lib/types';

export function listRules() {
  return invoke<AutomationRule[]>('list_rules');
}

export function saveRule(rule: AutomationRule) {
  return invoke<AutomationRule>('save_rule', { rule });
}

export function testRule(rule: AutomationRule) {
  return invoke<RuleMatchExplanation[]>('test_rule', { rule });
}

export function deleteRule(id: string) {
  return invoke<void>('delete_rule', { id });
}
