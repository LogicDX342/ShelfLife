import { invoke } from '@tauri-apps/api/core';

import type { RuleMatchExplanation, TrackedFile } from '$lib/types';

export function getActiveFiles() {
  return invoke<TrackedFile[]>('get_active_files');
}

export function explainFile(path: string) {
  return invoke<RuleMatchExplanation[]>('explain_file', { path });
}

export function openFileLocation(path: string) {
  return invoke<void>('open_file_location', { path });
}

export function selectDirectory(title?: string, defaultPath?: string) {
  return invoke<string | null>('select_directory', { title, defaultPath });
}

export function filterExistingDirectories(paths: string[]) {
  return invoke<string[]>('filter_existing_directories', { paths });
}
