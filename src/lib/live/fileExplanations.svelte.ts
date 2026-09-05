import { explainFiles } from '$lib/api/files';
import { i18n } from '$lib/i18n/i18n.svelte';
import { notifications } from '$lib/stores/notifications.svelte';
import type { RuleMatchExplanation, TrackedFile } from '$lib/types';
import { getErrorMessage } from '$lib/utils/format';

export function createFileExplanations(getVisibleFiles: () => TrackedFile[]) {
  let byPath = $state<Record<string, RuleMatchExplanation[]>>({});

  $effect(() => {
    const paths = getVisibleFiles().map((file) => file.path);
    if (paths.length === 0) {
      byPath = {};
      return;
    }

    let cancelled = false;
    explainFiles(paths)
      .then((explanations) => {
        if (!cancelled) byPath = explanations;
      })
      .catch((reason) => {
        if (!cancelled) {
          notifications.error(getErrorMessage(reason, i18n.t('file.errorExplanation')));
        }
      });
    return () => {
      cancelled = true;
    };
  });

  return {
    get byPath() {
      return byPath;
    },
  };
}
