<script lang="ts">
  import IconCircleHelp from '@lucide/svelte/icons/circle-help';

  import * as Tooltip from '$lib/components/ui/tooltip';

  let { content, contentClass = 'max-w-64 leading-relaxed' } = $props<{
    content: string;
    contentClass?: string;
  }>();
</script>

<Tooltip.Root>
  <Tooltip.Trigger>
    <IconCircleHelp
      class="w-3.5 h-3.5 text-fluent-muted-light dark:text-fluent-muted-dark hover:text-fluent-text-light dark:hover:text-fluent-text-dark transition-colors cursor-help"
    />
  </Tooltip.Trigger>
  <Tooltip.Content class={contentClass}>
    {content}
  </Tooltip.Content>
</Tooltip.Root>
