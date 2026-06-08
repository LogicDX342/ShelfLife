import { listRules } from '$lib/api/rules';
import type { AutomationRule } from '$lib/types';
import { getErrorMessage } from '$lib/utils/format';

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
      this.error = getErrorMessage(error, 'Could not load rules.');
    } finally {
      this.loading = false;
    }
  }
}

export const rulesState = new RulesState();
