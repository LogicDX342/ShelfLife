<script lang="ts">
  import { Spinner } from '$lib/components/ui/spinner';

  let { label } = $props<{
    label: string;
  }>();
</script>

<div class="py-12 flex flex-col items-center justify-center gap-3">
  <Spinner class="size-8" />
  <span class="text-sm text-muted-foreground">{label}</span>
</div>
