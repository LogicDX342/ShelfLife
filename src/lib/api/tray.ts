import { invoke } from '@tauri-apps/api/core';

import type { TrayLabels } from '$lib/types';

export function updateTrayLabels(labels: TrayLabels) {
  return invoke<void>('update_tray_labels', { labels });
}
