# Stores

Each store is a class in a `.svelte.ts` file built from `Resource`
(`resource.svelte.ts`): one value fetched from the daemon for a key, with
`loading`, `error` and `loaded` runes.

The pattern, copied by every store:

1. **Construct with a `Send`.** `createFooStore(send = defaultSend)` returns a
   new store. A store of what every page shares (workspaces, feeds,
   connectors, extensions) also exports one shared instance; a
   store of what one page has open (a task list, a document, a conversation,
   an actor session, a workspace's tags) exports none, and joins
   `PageStores` (`pageStores.svelte.ts`) instead, which gives each half of a
   split view its own set: a component takes its half's with
   `const { tasks } = pageStores()`, and the shell gets the main half's.
   Tests pass `createMock().send`.
2. **One `Resource` per fetched value.** Give it `fetch(key)`, an `initial`
   value, and `matches(invalidation, key)` saying which `invalidate` pushes
   touch it. `canRefetch()` can veto background refetches (the journal does
   while it has unsaved edits).
3. **`load(key)`** fetches unless that key is already loaded; a different key
   resets the value first, unless the resource's `keepsValue(from, to)` lets
   the loaded value show until the new one lands (the task list's filters,
   order and page swap in place rather than emptying). Every `load` makes
   the resource _active_, even
   one that returns at once because the key is loaded.
4. **Mutations update from the reply.** Send the request, then
   `resource.update(...)` or `resource.set(...)` with what came back. Never
   refetch after your own mutation, unless the reply lacks something only
   the daemon works out (a listed document's `opening`); then call
   `refresh()`, which shares the fetch the mutation's invalidation starts,
   rather than working it out in the store. Mutations throw `ProtocolError`; the
   caller decides whether to toast or show a field error (`error.field`).
5. **Background refetch is automatic.** The layout calls `refreshAll()` on
   window focus, `reset` and reconnect, and `refreshInvalidated(keys)` on
   `invalidate`. Both only touch active resources. A refresh that lands while
   a fetch is in flight queues one more fetch after it, shared by every
   change that lands meanwhile, so a burst of invalidations costs two
   fetches, not one each. A resource holding a long list can take `patch`
   for keys that name one row (`documentText`, `taskDescription`): it
   fetches just those rows and folds them in, instead of the whole list.
6. **Release what a screen stops showing.** A component that loads a
   resource only it shows calls `release()` when it unmounts, so hidden data
   isn't refetched on every focus. The value stays, so coming back renders at
   once; the next `load` makes it active again. With the same key it fetches
   only if a matching `invalidate` or a `refreshAll` arrived while it was
   released; otherwise it just rejoins background refetch. Resources the
   shell shows too (the workspace list) are never released.

```svelte
<script lang="ts">
	$effect(() => {
		untrack(() => void clients.load());
		return () => clients.list.release();
	});
</script>
```

```ts
export class TasksStore {
	readonly list: Resource<Task[], number>;
	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: (workspace_id) => send('listTasks', { workspace_id }),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'workspace' && key.id === workspace_id
		});
	}

	async rename(task_id: number, title: string) {
		const workspace_id = this.list.key!;
		const task = await this.#send('renameTask', { workspace_id, task_id, title });
		this.list.update((list) => list.map((t) => (t.id === task.id ? task : t)));
	}
}

export function createTasksStore(send: Send = defaultSend): TasksStore {
	return new TasksStore(send);
}
```
