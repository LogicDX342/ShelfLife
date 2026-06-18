import { invoke } from '@tauri-apps/api/core';

import type { DropzoneActionResult, DropzonePreview } from '$lib/types';

export function previewDropzoneFiles(paths: string[]) {
  return invoke<DropzonePreview>('preview_dropzone_files', { paths });
}

export function executeDropzoneIngest(paths: string[], watchTargetId: string) {
  return invoke<DropzoneActionResult>('execute_dropzone_ingest', {
    paths,
    watchTargetId,
  });
}

export function executeDropzoneRuleGroup(ruleId: string, paths: string[]) {
  return invoke<DropzoneActionResult>('execute_dropzone_rule_group', {
    ruleId,
    paths,
  });
}

export function hideDropzone() {
  return invoke<void>('hide_dropzone');
}
