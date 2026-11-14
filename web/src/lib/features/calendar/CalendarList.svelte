<!--
	Every calendar, grouped by the account (calendar store) that holds it: Cohesion's
	own, ones made in Cohesion, and connected ones. Each calendar has a box in its
	color that shows or hides it in this workspace (its perspective), and a ⋯ menu:
	its color, and for one made in Cohesion Rename and Delete. An account made in
	Cohesion ends with New calendar; a connected one says when it last synced, or why
	it failed. An account's ⋯ menu renames it (or gives it back its own name), and
	makes a calendar in it and deletes it, or opens the connected account. At the top,
	Bypass perspective draws every calendar for this visit; at the foot, New account
	and a way to connect one. The column right of the sheet draws it beside the
	calendar page; below that width the page opens it in a side panel.
-->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import Plus from '@lucide/svelte/icons/plus';
	import { ago } from '$lib/format/dates';
	import type { Calendar, CalendarStore, IntelligenceAgentColor } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import { routes } from '$lib/shell/routes';
	import { calendars } from '$lib/stores/calendars.svelte';
	import { agentFill } from '$lib/ui/colors';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import GutterCard from '$lib/ui/GutterCard.svelte';
	import GutterGroup from '$lib/ui/GutterGroup.svelte';
	import type { MenuItem } from '$lib/ui/Menu.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import Switch from '$lib/ui/Switch.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import NameDialog from './NameDialog.svelte';
	import { isDrawn, perspectiveBypass } from './perspective.svelte';

	interface Props {
		/** The workspace whose perspective the boxes show and change. */
		workspaceId: number | null;
		/** Drawn inside a side panel, so its menus must sit above it. */
		inDialog?: boolean;
		/** Follows a link out of the list (the panel closes on it). */
		onleave?: () => void;
		/** Heads it with a summary as tall as the page's header, as the column right of the sheet does. */
		summary?: boolean;
	}

	let { workspaceId, inDialog = false, onleave, summary = false }: Props = $props();

	const here = pane();

	$effect(() => {
		if (workspaceId !== null) void calendars.perspective.load(workspaceId);
	});

	const bypassed = $derived(workspaceId !== null && perspectiveBypass.isOn(workspaceId));
	const shownCount = $derived(
		workspaceId === null
			? 0
			: calendars.calendars.value.filter((c) => isDrawn(workspaceId, c.id)).length
	);

	const stores = $derived(calendars.stores.value);
	const loaded = $derived(calendars.stores.loaded && calendars.calendars.loaded);
	const calendarsOf = (store: CalendarStore) =>
		calendars.calendars.value.filter((c) => c.store_id === store.id);

	const run = (work: Promise<unknown>) => work.catch((e) => toasts.error(e));

	function setShown(calendar: Calendar, shown: boolean) {
		if (workspaceId === null) return;
		run(calendars.setInPerspective(workspaceId, calendar.id, shown));
	}

	// ---- Dialogs ------------------------------------------------------------

	let naming = $state<
		| { kind: 'store' }
		| { kind: 'renameStore'; store: CalendarStore }
		| { kind: 'calendar'; store: CalendarStore }
		| { kind: 'rename'; calendar: Calendar }
		| null
	>(null);
	let namingOpen = $state(false);
	let deletingCalendar = $state<Calendar | null>(null);
	let deletingStore = $state<CalendarStore | null>(null);

	function name(next: NonNullable<typeof naming>) {
		naming = next;
		namingOpen = true;
	}

	/** The first palette color no calendar of `store` has yet. */
	function freshColor(store: CalendarStore): IntelligenceAgentColor {
		const taken = new Set(calendarsOf(store).map((c) => c.selected_color));
		const palette = vocabulary.value.agent_colors.map((c) => c.kind);
		return palette.find((c) => !taken.has(c)) ?? palette[0] ?? 'blue';
	}

	function submitName(title: string): Promise<unknown> {
		const n = naming;
		if (n?.kind === 'store') return calendars.createStore(title);
		if (n?.kind === 'renameStore') return calendars.renameStore(n.store.id, title);
		if (n?.kind === 'calendar')
			return calendars.create(
				n.store.id,
				title,
				{ kind: 'palette', color: freshColor(n.store) },
				workspaceId
			);
		if (n?.kind === 'rename') return calendars.rename(n.calendar.id, title);
		return Promise.resolve();
	}

	const dialogText = $derived.by(() => {
		if (naming?.kind === 'rename')
			return { title: 'Rename calendar', submit: 'Rename', initial: naming.calendar.title };
		if (naming?.kind === 'renameStore')
			return { title: 'Rename account', submit: 'Rename', initial: naming.store.title };
		if (naming?.kind === 'calendar')
			return { title: `New calendar in ${naming.store.title}`, submit: 'Create', initial: '' };
		return { title: 'New account', submit: 'Create', initial: '' };
	});

	// ---- Menus --------------------------------------------------------------

	function storeItems(store: CalendarStore): MenuItem[] {
		const naming: MenuItem[] = [
			{ label: 'Rename…', onSelect: () => name({ kind: 'renameStore', store }) }
		];
		if (store.named_by_user)
			naming.push({
				label: store.backend.kind === 'local' ? 'Name it Cohesion' : 'Use the account’s name',
				onSelect: () => run(calendars.renameStore(store.id, null))
			});
		if (store.backend.kind === 'account')
			return [
				...naming,
				{
					label: 'Open account',
					separated: true,
					onSelect: () => {
						onleave?.();
						void here.goto(routes.connectorAccounts());
					}
				}
			];
		return [
			{ label: 'New calendar…', onSelect: () => name({ kind: 'calendar', store }) },
			...naming.map((item, i) => (i === 0 ? { ...item, separated: true } : item)),
			{
				label: 'Delete account…',
				danger: true,
				separated: true,
				onSelect: () => (deletingStore = store)
			}
		];
	}

	function calendarItems(calendar: Calendar, store: CalendarStore | undefined): MenuItem[] {
		const items: MenuItem[] = [
			{
				label: 'Color',
				items: vocabulary.value.agent_colors.map(({ kind, label }) => ({
					label,
					ariaLabel: `Color: ${label}`,
					checked: calendar.selected_color === kind,
					onSelect: () =>
						run(
							calendars.setColor(calendar.id, {
								kind: 'palette',
								color: kind as IntelligenceAgentColor
							})
						)
				}))
			}
		];
		if (store?.backend.kind === 'local')
			items.push(
				{
					label: 'Rename…',
					separated: true,
					onSelect: () => name({ kind: 'rename', calendar })
				},
				{ label: 'Delete…', danger: true, onSelect: () => (deletingCalendar = calendar) }
			);
		return items;
	}

	function syncLine(store: CalendarStore): { text: string; failed: boolean } | null {
		if (store.backend.kind === 'local') return null;
		if (store.sync_failure) return { text: `Sync failed: ${store.sync_failure}`, failed: true };
		if (store.last_synced_at === null) return { text: 'Not synced yet', failed: false };
		return { text: `Synced ${ago(store.last_synced_at)}`, failed: false };
	}

	const footLink =
		'flex cursor-pointer items-center gap-2 rounded-sm py-1 text-left text-sm text-fg-muted no-underline hover:text-fg hover:no-underline';
</script>

{#if summary}
	<GutterCard>
		<p class="truncate text-sm text-fg">Calendars</p>
		{#if loaded && calendars.calendars.value.length > 0}
			<p class="truncate text-xs text-fg-muted">
				{shownCount} of {calendars.calendars.value.length} shown
			</p>
		{/if}
	</GutterCard>
{/if}

{#if loaded && workspaceId !== null && calendars.calendars.value.length > 0}
	{@const id = workspaceId}
	<GutterGroup>
		<label class="flex min-h-7 cursor-pointer items-center justify-between gap-2 text-sm text-fg">
			<span class="min-w-0">
				<span class="block truncate">Bypass perspective</span>
				<span class="block text-xs text-fg-subtle">Show every calendar in Cohesion</span>
			</span>
			<Switch checked={bypassed} onchange={(on) => perspectiveBypass.set(id, on)} />
		</label>
	</GutterGroup>
{/if}

{#if loaded && stores.length === 0}
	<EmptyState
		compact
		message="No accounts"
		detail="Create an account for your own calendars, or connect one that has calendars."
	/>
{/if}

{#each stores as store (store.id)}
	{@const sync = syncLine(store)}
	<GutterGroup>
		<div class="flex min-h-7 items-center justify-between gap-2" data-menu-row>
			<h2 class="min-w-0 truncate text-xs font-medium text-fg-subtle">{store.title}</h2>
			<RowMenu label="More for {store.title}" items={storeItems(store)} {inDialog} />
		</div>
		{#if sync}
			<p class={['-mt-1.5 text-xs', sync.failed ? 'text-warn' : 'text-fg-subtle']}>{sync.text}</p>
		{/if}
		<ul class="flex flex-col">
			{#each calendarsOf(store) as calendar (calendar.id)}
				{@const shown = workspaceId !== null && calendars.inPerspective(calendar.id)}
				<li class="flex min-h-7 items-center gap-1">
					<button
						class="flex min-w-0 flex-1 cursor-pointer items-center gap-2 py-1 text-left text-sm text-fg-muted hover:text-fg"
						type="button"
						role="checkbox"
						aria-checked={shown}
						onclick={() => setShown(calendar, !shown)}
					>
						<span
							class={[
								'grid size-3.5 flex-none place-items-center rounded-[4px] border-2',
								shown
									? [agentFill[calendar.selected_color], 'border-transparent']
									: 'border-border-strong'
							]}
							aria-hidden="true"
						>
							{#if shown}<Check class="text-accent-fg" size={10} strokeWidth={3.5} />{/if}
						</span>
						<span class={['min-w-0 truncate', shown && 'text-fg']}>{calendar.title}</span>
						{#if calendar.read_only}<span class="flex-none text-xs text-fg-subtle">Read-only</span
							>{/if}
					</button>
					<RowMenu
						label="More for {calendar.title}"
						items={calendarItems(calendar, store)}
						{inDialog}
					/>
				</li>
			{:else}
				{#if store.backend.kind === 'account'}
					<li class="py-1 text-sm text-fg-subtle">No calendars</li>
				{/if}
			{/each}
			{#if store.backend.kind === 'local'}
				<li>
					<button class={footLink} type="button" onclick={() => name({ kind: 'calendar', store })}>
						<Plus size={14} strokeWidth={1.75} />New calendar…
					</button>
				</li>
			{/if}
		</ul>
	</GutterGroup>
{/each}

{#if loaded}
	<GutterGroup>
		<button class={footLink} type="button" onclick={() => name({ kind: 'store' })}>
			<Plus size={14} strokeWidth={1.75} />New account…
		</button>
		<a class={footLink} href={routes.connectorAccounts()} onclick={() => onleave?.()}>
			<Plus size={14} strokeWidth={1.75} />Connect an account
		</a>
	</GutterGroup>
{/if}

<NameDialog
	bind:open={namingOpen}
	title={dialogText.title}
	initial={dialogText.initial}
	submitLabel={dialogText.submit}
	onsubmit={submitName}
/>
<ConfirmDialog
	subject={deletingCalendar}
	onclose={() => (deletingCalendar = null)}
	title="Delete “{deletingCalendar?.title ?? ''}”?"
	description="All events in this calendar will also be deleted."
	confirmLabel="Delete"
	onconfirm={(calendar) => calendars.remove(calendar.id)}
/>
<ConfirmDialog
	subject={deletingStore}
	onclose={() => (deletingStore = null)}
	title="Delete “{deletingStore?.title ?? ''}”?"
	description="All calendars and events in this account will also be deleted."
	confirmLabel="Delete"
	onconfirm={(store) => calendars.deleteStore(store.id)}
/>
