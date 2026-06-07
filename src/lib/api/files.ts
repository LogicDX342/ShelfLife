import { invoke } from '@tauri-apps/api/core';
import type { FilePreview, RuleMatchExplanation, TrackedFile } from '$lib/types';

export function getActiveFiles() {
  return invoke<TrackedFile[]>('get_active_files');
}

export function explainFile(path: string) {
  return invoke<RuleMatchExplanation[]>('explain_file', { path });
}

export function previewFile(path: string) {
  return invoke<FilePreview>('preview_file', { path });
}

export function openFileLocation(path: string) {
  return invoke<void>('open_file_location', { path });
}

export function selectDirectory(title?: string, defaultPath?: string) {
  return invoke<string | null>('select_directory', { title, defaultPath });
}
