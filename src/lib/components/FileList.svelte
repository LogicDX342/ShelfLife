<script lang="ts">
  import type { FileDecayState, TrackedFile } from '$lib/types';
  import FileCard from './FileCard.svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';

  let {
    files = [],
    onRefresh,
    selectedPaths = [],
    onSelectedChange = () => {},
  } = $props<{
    files: TrackedFile[];
    onRefresh: () => Promise<void>;
    selectedPaths?: string[];
    onSelectedChange?: (path: string, selected: boolean) => void;
  }>();

  const states: FileDecayState[] = ['Decaying', 'Stale', 'Fresh', 'Pinned', 'Ignored'];

  let grouped = $derived(
    states.map((state) => ({
      state,
      files: files.filter((file: TrackedFile) => file.state === state),
    })),
  );

  // Default collapse state - collapse Pinned and Ignored by default to reduce noise
  let collapsedGroups = $state<Record<string, boolean>>({
    Decaying: false,
    Stale: false,
    Fresh: false,
    Pinned: true,
    Ignored: true,
  });

  let visibleLimits = $state<Record<string, number>>({
    Decaying: 50,
    Stale: 50,
    Fresh: 50,
    Pinned: 50,
    Ignored: 50,
  });

  function toggleGroup(state: FileDecayState) {
    collapsedGroups[state] = !collapsedGroups[state];
  }

  // Get accent color for state dots
  function getStateColors(state: FileDecayState) {
    switch (state) {
      case 'Fresh':
        return 'bg-green-500 text-green-500 border-green-500/20';
      case 'Stale':
        return 'bg-amber-500 text-amber-500 border-amber-500/20';
      case 'Decaying':
        return 'bg-red-500 text-red-500 border-red-500/20';
      case 'Pinned':
        return 'bg-blue-500 text-blue-500 border-blue-500/20';
      case 'Ignored':
        return 'bg-neutral-400 text-neutral-400 border-neutral-400/20';
      default:
        return 'bg-neutral-500 text-neutral-500 border-neutral-500/20';
    }
  }
</script>

<div class="space-y-6">
  {#each grouped as group (group.state)}
    {#if group.files.length > 0}
      <section
        class="fluent-card p-0 overflow-hidden bg-fluent-card-light dark:bg-fluent-card-dark"
      >
        <!-- Collapsible Header -->
        <button
          onclick={() => toggleGroup(group.state)}
          class="w-full flex items-center justify-between px-4 py-3 bg-black/2.5 dark:bg-white/2.5 hover:bg-black/5 dark:hover:bg-white/5 border-b border-fluent-border-light dark:border-fluent-border-dark text-left transition-colors cursor-pointer select-none"
        >
          <div class="flex items-center gap-3">
            <!-- State dot indicator -->
            <span class="flex h-2.5 w-2.5 relative">
              {#if group.state === 'Decaying' || group.state === 'Stale'}
                <span
                  class="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75 {getStateColors(
                    group.state,
                  ).split(' ')[0]}"
                ></span>
              {/if}
              <span
                class="relative inline-flex rounded-full h-2.5 w-2.5 {getStateColors(
                  group.state,
                ).split(' ')[0]}"
              ></span>
            </span>
            <span class="font-semibold text-sm">
              {i18n.t(`tab.${group.state.toLowerCase()}`)}
            </span>
            <span
              class="px-2 py-0.5 text-xs bg-black/10 dark:bg-white/10 rounded-full font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >
              {group.files.length}
            </span>
          </div>

          <!-- Chevron Icon -->
          <svg
            class="w-4 h-4 text-fluent-muted-light dark:text-fluent-muted-dark transform transition-transform duration-200 {collapsedGroups[
              group.state
            ]
              ? ''
              : 'rotate-180'}"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2.5"
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </button>

        <!-- Collapsible Content -->
        {#if !collapsedGroups[group.state]}
          <div class="p-4 space-y-4 bg-transparent">
            {#each group.files.slice(0, visibleLimits[group.state]) as file (file.path)}
              <FileCard
                {file}
                {onRefresh}
                selectable
                selected={selectedPaths.includes(file.path)}
                {onSelectedChange}
              />
            {/each}

            {#if group.files.length > visibleLimits[group.state]}
              <div class="pt-2 flex justify-center">
                <button
                  type="button"
                  class="fluent-button w-full justify-center text-xs font-semibold py-2"
                  onclick={() => {
                    visibleLimits[group.state] = (visibleLimits[group.state] || 50) + 100;
                  }}
                >
                  Load More ({group.files.length - visibleLimits[group.state]} remaining)
                </button>
              </div>
            {/if}
          </div>
        {/if}
      </section>
    {/if}
  {/each}
</div>
