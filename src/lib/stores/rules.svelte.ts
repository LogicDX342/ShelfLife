import { listRules } from '$lib/api/rules';
import type { AutomationRule } from '$lib/types';

class RulesState {
  rules = $state<AutomationRule[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);

  async refresh() {
    this.loading = true;
    this.error = null;
    try {
      this.rules = await listRules();
    } catch (error) {
      this.error = error instanceof Error ? error.message : 'Could not load rules.';
    } finally {
      this.loading = false;
    }
  }
}

export const rulesState = new RulesState();
