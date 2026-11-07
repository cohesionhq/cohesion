<!-- Where a `cohesion://` link naming its workspace lands (`cohesion://task/3/42`): only that workspace is looked in. -->
<script lang="ts">
	import { workspaceItemRoute } from '$lib/shell/links';
	import OpenItem from '$lib/shell/OpenItem.svelte';
	import type { WorkspaceItemKind } from '$lib/shell/routes';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	const kind = $derived(page.params.kind as WorkspaceItemKind);
	const ws = $derived(Number(page.params.ws));
	const id = $derived(Number(page.params.id));
</script>

<OpenItem {kind} find={() => workspaceItemRoute(kind, ws, id)} />
