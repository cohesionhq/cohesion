<!-- Where a `cohesion://` link without a workspace lands (`cohesion://task/42`, `cohesion://agent/3`). -->
<script lang="ts">
	import { itemRoute } from '$lib/shell/links';
	import OpenItem from '$lib/shell/OpenItem.svelte';
	import type { ItemKind } from '$lib/shell/routes';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	const kind = $derived(page.params.kind as ItemKind);
	const id = $derived(Number(page.params.id));
</script>

<OpenItem {kind} find={() => itemRoute(kind, id)} />
