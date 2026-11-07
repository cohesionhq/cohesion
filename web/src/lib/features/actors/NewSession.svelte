<!--
	Hands an actor a new session, as the page's centrepiece: a heading asking
	what the picked actor should work on, and under it what it is, the
	device it runs on and where it works; then what to do, in a
	composer lit by a soft glow of the accent, with a menu bar of chips
	for the actor, its model and effort and, when the actor has a choice of
	them, the mode, and under it a reminder that agents can start
	sessions too. What to do is kept in this browser until the session starts.
	It starts on the actor, model and effort last picked in
	this browser, while that actor is online and still offers them, else
	on the first online actor and its defaults, and opens the session once
	started.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Actor, ImageUpload } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { rememberPick, rememberedPick } from '$lib/stores/actors.svelte';
	import { rememberText, rememberedText } from '$lib/stores/drafts';
	import Choices from '$lib/ui/Choices.svelte';
	import MenuBar from '$lib/ui/MenuBar.svelte';
	import MenuBarItem from '$lib/ui/MenuBarItem.svelte';
	import ActorPicker from './ActorPicker.svelte';
	import ActorPlace from './ActorPlace.svelte';
	import ModelPicker from './ModelPicker.svelte';
	import SessionComposer from './SessionComposer.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { actors: store } = pageStores();

	const page = pane();

	interface Props {
		actors: Actor[];
		/** Whether client `id` is online, as the clients list says. */
		online: (id: number) => boolean;
	}

	let { actors, online }: Props = $props();

	const remembered = rememberedPick();

	let actorId = $state<number | null>(null);
	const DRAFT_KEY = 'newActorSession';

	let prompt = $state(rememberedText(DRAFT_KEY));
	let images = $state<ImageUpload[]>([]);
	let mode = $state('');
	let model = $state(remembered?.model ?? '');
	let effort = $state(remembered?.effort ?? '');
	let busy = $state(false);
	let error = $state<string | null>(null);

	$effect(() => rememberText(DRAFT_KEY, prompt));

	const actor = $derived(actors.find((a) => a.id === actorId) ?? null);
	const modeTitle = $derived(actor?.modes.find((m) => m.identifier === mode)?.title ?? '');

	// Start on the actor last picked, else the first online one, and each
	// actor on its own defaults where it doesn't offer what was picked.
	$effect(() => {
		if (actorId !== null) return;
		const reachable = actors.filter((a) => online(a.client_id));
		actorId = (reachable.find((a) => a.id === remembered?.actor_id) ?? reachable[0])?.id ?? null;
	});
	let defaultsFor: number | null = null;
	$effect(() => {
		if (actor && defaultsFor !== actor.id) settle(actor);
	});

	/** Keeps the model and effort where `next` offers them, else takes its defaults. */
	function settle(next: Actor) {
		defaultsFor = next.id;
		mode = next.modes[0]?.identifier ?? '';
		if (!next.models.some((m) => m.identifier === model)) model = next.models[0]?.identifier ?? '';
		const efforts = next.models.find((m) => m.identifier === model)?.efforts ?? [];
		if (!efforts.some((e) => e.identifier === effort)) effort = '';
	}

	function pickActor(next: Actor) {
		actorId = next.id;
		settle(next);
		remember();
	}

	/** Remembers what the user picked, so the next new session starts there. */
	function remember() {
		if (actorId !== null) rememberPick({ actor_id: actorId, model, effort });
	}

	async function start() {
		if (!actor || busy) return;
		busy = true;
		error = null;
		try {
			const session = await store.start({
				actor_id: actor.id,
				prompt,
				mode: mode || null,
				model: model || null,
				effort: effort || null,
				attachments: images.length > 0 ? images : null
			});
			prompt = '';
			images = [];
			await page.goto(routes.actorSession(session.id));
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			busy = false;
		}
	}
</script>

<section
	class="flex flex-col items-center gap-6 py-10 max-sm:py-4"
	aria-labelledby="new-session-heading"
>
	<div class="flex flex-col items-center gap-3 text-center">
		<h2 id="new-session-heading" class="text-xl leading-tight font-semibold tracking-tight">
			What should {actor?.product ?? 'an actor'} work on?
		</h2>
		{#if actor}
			<p class="text-sm text-fg-muted">
				<ActorPlace {actor} pathClass="text-fg" />
			</p>
		{/if}
	</div>

	<!-- The glow: blurred washes of the accent and two agent colors, behind the capsule. -->
	<div class="relative isolate w-full">
		<div
			class="glow pointer-events-none absolute -inset-x-6 -inset-y-4 -z-1 rounded-[2rem] blur-2xl"
			aria-hidden="true"
		></div>
		<SessionComposer
			bind:value={prompt}
			bind:images
			disabled={busy || !actor || !online(actor.client_id)}
			placeholder={actor ? 'Describe the session…' : 'No actor is online.'}
			onsend={start}
			{error}
		>
			{#snippet tools()}
				<MenuBar>
					<ActorPicker {actors} {online} {actorId} onpick={pickActor} />
					{#if actor && actor.models.length > 0}
						<ModelPicker models={actor.models} bind:model bind:effort onpick={remember} />
					{/if}
					{#if actor && actor.modes.length > 1}
						<MenuBarItem label={modeTitle} name="Mode">
							{#snippet children(close)}
								<Choices
									label="Mode"
									choices={actor.modes.map((m) => ({
										id: m.identifier,
										label: m.title,
										detail: m.description ?? undefined
									}))}
									checked={mode}
									onpick={(id) => {
										mode = String(id);
										close();
									}}
								/>
							{/snippet}
						</MenuBarItem>
					{/if}
				</MenuBar>
			{/snippet}
		</SessionComposer>
	</div>
	<p class="text-center text-xs text-fg-subtle">
		You can also ask your agents to start sessions on actors.
	</p>
</section>

<style>
	.glow {
		background:
			radial-gradient(40% 70% at 20% 40%, var(--accent), transparent 70%),
			radial-gradient(40% 70% at 80% 60%, var(--agent-purple), transparent 70%),
			radial-gradient(30% 60% at 55% 20%, var(--agent-blue), transparent 70%);
		opacity: 0.4;
	}
</style>
