import { Channel, invoke } from '@tauri-apps/api/core';

import type { AppUpdate, AppUpdateEvent } from '$lib/types';

export function checkForUpdate() {
  return invoke<AppUpdate | null>('check_for_update');
}

export function installUpdate(onEvent: (event: AppUpdateEvent) => void) {
  return invoke<void>('install_update', { onEvent: new Channel<AppUpdateEvent>(onEvent) });
}
