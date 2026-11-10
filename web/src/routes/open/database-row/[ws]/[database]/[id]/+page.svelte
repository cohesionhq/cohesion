<!-- Where a link to a database row lands (`cohesion://database-row/3/2/14`): its database's table, with the row open over it. -->
<script lang="ts">
	import { itemRoute } from '$lib/shell/links';
	import OpenItem from '$lib/shell/OpenItem.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const database = $derived(Number(page.params.database));
	const id = $derived(Number(page.params.id));
</script>

<OpenItem kind="database-row" find={() => itemRoute({ kind: 'database-row', ws, database, id })} />
