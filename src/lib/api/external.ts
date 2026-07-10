import { invoke } from '@tauri-apps/api/core';

export const externalUrls = {
  github: 'https://github.com/LogicDX342/ShelfLife',
  bugReport: 'https://github.com/LogicDX342/ShelfLife/issues/new/choose',
} as const;

export type ExternalUrl = (typeof externalUrls)[keyof typeof externalUrls];

export function openExternalUrl(url: ExternalUrl) {
  return invoke<void>('open_external_url', { url });
}
