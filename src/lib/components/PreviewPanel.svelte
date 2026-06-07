<script lang="ts">
  import { previewFile } from '$lib/api/files';
  import type { FilePreview } from '$lib/types';
  import { onMount } from 'svelte';

  let { path } = $props<{ path: string }>();
  let preview = $state<FilePreview | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function load() {
    if (preview || error || loading) return;
    loading = true;
    try {
      preview = await previewFile(path);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : 'Preview unavailable';
    } finally {
      loading = false;
    }
  }

  // Load immediately on mount as well to ensure snappy UX when details are opened
  onMount(() => {
    load();
  });
</script>

<section
  class="bg-neutral-50 dark:bg-neutral-900 border border-fluent-border-light dark:border-fluent-border-dark rounded-md p-3.5 min-h-[5rem] max-h-64 overflow-auto text-xs transition-colors duration-150"
  aria-label="File preview"
  onmouseenter={load}
  onfocusin={load}
>
  {#if loading}
    <div class="flex items-center gap-2 text-fluent-muted-light dark:text-fluent-muted-dark py-2">
      <svg class="animate-spin h-3.5 w-3.5 text-fluent-accent" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
        ></circle>
        <path
          class="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
        ></path>
      </svg>
      <span>Loading preview...</span>
    </div>
  {:else if error}
    <span class="text-fluent-muted-light dark:text-fluent-muted-dark italic">{error}</span>
  {:else if preview && typeof preview.content !== 'string' && 'Text' in preview.content}
    <div class="space-y-2">
      <pre
        class="font-mono text-xs leading-relaxed text-neutral-800 dark:text-neutral-200 whitespace-pre-wrap break-all select-text">{preview
          .content.Text.snippet}</pre>
      {#if preview.content.Text.truncated}
        <div
          class="text-[10px] text-fluent-accent font-semibold tracking-wider uppercase pt-2 border-t border-neutral-200/50 dark:border-neutral-800/50"
        >
          Preview truncated · Full file not loaded
        </div>
      {/if}
    </div>
  {:else if preview && typeof preview.content !== 'string' && 'Image' in preview.content}
    <div class="flex items-center gap-3 py-1">
      <div class="p-2 bg-neutral-200 dark:bg-neutral-800 rounded">
        <!-- SVG Image icon -->
        <svg
          class="w-8 h-8 text-fluent-muted-light dark:text-fluent-muted-dark"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="1.8"
            d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
          />
        </svg>
      </div>
      <div class="flex flex-col">
        <span class="font-semibold text-neutral-700 dark:text-neutral-300">Image file</span>
        <span class="text-fluent-muted-light dark:text-fluent-muted-dark text-[10px]">
          {preview.content.Image.format.toUpperCase()} · {preview.content.Image.width} × {preview
            .content.Image.height}px
        </span>
      </div>
    </div>
  {:else if preview && typeof preview.content !== 'string' && 'Pdf' in preview.content}
    <div class="flex items-center gap-3 py-1">
      <div class="p-2 bg-neutral-200 dark:bg-neutral-800 rounded">
        <!-- SVG PDF icon -->
        <svg
          class="w-8 h-8 text-fluent-muted-light dark:text-fluent-muted-dark"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="1.8"
            d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"
          />
        </svg>
      </div>
      <div class="flex flex-col">
        <span class="font-semibold text-neutral-700 dark:text-neutral-300">PDF Document</span>
        <span class="text-fluent-muted-light dark:text-fluent-muted-dark text-[10px]">
          {preview.content.Pdf.page_count === null
            ? 'Unknown pages'
            : `${preview.content.Pdf.page_count} pages`}
          {preview.content.Pdf.title === null ? '' : ` · "${preview.content.Pdf.title}"`}
        </span>
      </div>
    </div>
  {:else}
    <div class="flex items-center gap-2 text-fluent-muted-light dark:text-fluent-muted-dark py-2">
      <svg class="w-5 h-5 opacity-60" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="1.8"
          d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
      <span>{preview?.mime_type ?? 'No content preview available'}</span>
    </div>
  {/if}
</section>
