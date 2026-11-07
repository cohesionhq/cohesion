<!--
	One actor session, drawn like a coding agent's session: a pinned header
	with where it stands, then its transcript (what the user said in bubbles
	on the right; the actor's words flat across the column, the tools it ran
	as one quiet line each, stacked tight, with an icon for its category (a spinner
	while it runs) and how long each took, and its
	requests as cards, each answerable until answered; a question's card
	holds its answers; images the user sent sit over their bubble, and those
	the actor showed under what it said), and a
	floating composer for what to say next, images too, while its client is
	online. A session that is done or failed picks back up on a
	message. Under the composer float the session's controls: when the client
	reports it, a ring showing how full the actor's context window is, which
	opens to its plan's limits; while the client is online, a mode chip that
	switches the mode it runs in, even mid-run; and Stop, which ends what it
	is doing. Resolve, in the header, says the user is done with it: it
	leaves the lists (one going on stops first, after asking), and a
	message picks it back up.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { elapsed, nowSeconds } from '$lib/format/dates';
	import type {
		ActorAnswer,
		ActorSessionMessage,
		ActorUserContent,
		ImageUpload
	} from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { isGoing } from '$lib/stores/actors.svelte';
	import { rememberText, rememberedText } from '$lib/stores/drafts';
	import { clients } from '$lib/stores/settings/clients.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import SentImages from '$lib/ui/SentImages.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import Shimmer from '$lib/ui/Shimmer.svelte';
	import WorkingLine from '$lib/ui/WorkingLine.svelte';
	import ModePicker from './ModePicker.svelte';
	import PermissionCard from './PermissionCard.svelte';
	import PlanCard from './PlanCard.svelte';
	import QuestionCard from './QuestionCard.svelte';
	import SessionComposer from './SessionComposer.svelte';
	import SessionHeader from './SessionHeader.svelte';
	import { planOf } from './plan';
	import { paneChrome } from '$lib/ui/paneChrome';
	import UsageRing from './UsageRing.svelte';
	import { stillRunning, toolIcon } from './tools';
	import { userText } from './words';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { actors } = pageStores();

	const page = pane();

	let { sessionId }: { sessionId: number } = $props();

	let draft = $state('');
	/** Where this browser keeps the session's unsent message, set with `draft` so neither runs ahead. */
	let draftKey = $state('');
	let images = $state<ImageUpload[]>([]);
	let stopping = $state(false);
	let resolving = $state(false);
	let scroller = $state<HTMLElement>();
	let content = $state<HTMLElement>();
	/** The floating composer's height, which the feed leaves clear at its foot. */
	let composerHeight = $state(0);

	$effect(() => {
		const id = sessionId;
		untrack(() => {
			draftKey = `actorSession.${id}`;
			draft = rememberedText(draftKey);
			images = [];
			void actors.session.load(id);
			void actors.messages.load(id);
			void actors.list.load();
			void actors.usage.load();
			void clients.load();
		});
		return () => {
			actors.session.release();
			actors.messages.release();
			actors.usage.release();
			clients.release();
		};
	});

	$effect(() => {
		if (draftKey) rememberText(draftKey, draft);
	});

	const session = $derived(actors.session.key === sessionId ? actors.session.value : null);
	const transcript = $derived(actors.messages.key === sessionId ? actors.messages.value : []);
	/** The user's answers to each question, by the question's id; they show in its card. */
	const responses = $derived(
		new Map(
			transcript.flatMap(({ content }): [number, ActorAnswer[]][] =>
				content.sender === 'user' && content.kind === 'answers'
					? [[content.request_id, content.answers]]
					: []
			)
		)
	);
	/** The transcript as the feed draws it, without the answers its question cards hold. */
	const messages = $derived(
		transcript.filter(({ content }) => !(content.sender === 'user' && content.kind === 'answers'))
	);
	const actor = $derived(
		(session && actors.list.value.find((a) => a.id === session.actor_id)) ?? null
	);
	const online = $derived(actor ? clients.isOnline(actor.client_id) : false);
	const plan = $derived(session ? planOf(actors.usage.value, session.actor_id) : []);
	/** From 1100px this half's usage shows in the column right of the sheet, not behind the ring. */
	const inGutter = paneChrome()?.hasGutter ? 'lg:hidden' : '';
	const missing = $derived(actors.session.error?.code === 'notFound');

	// An open session is a read one, including a stretch of work that ends while it's shown.
	$effect(() => {
		if (session?.unread) actors.markRead(session.id).catch((error) => toasts.error(error));
	});

	// The feed follows new messages while the reader is at (or near) the
	// bottom; scrolled up, it leaves them be.
	let following = true;
	let lastTop = 0;

	function toBottom() {
		if (scroller) scroller.scrollTop = scroller.scrollHeight;
	}

	function onscroll() {
		if (!scroller) return;
		const top = scroller.scrollTop;
		if (scroller.scrollHeight - top - scroller.clientHeight < 48) following = true;
		else if (top < lastTop) following = false;
		lastTop = top;
	}

	$effect(() => {
		void sessionId;
		following = true;
		lastTop = 0;
		untrack(toBottom);
	});

	$effect(() => {
		if (!scroller || !content) return;
		const observer = new ResizeObserver(() => {
			if (following) toBottom();
		});
		observer.observe(scroller);
		observer.observe(content);
		return () => observer.disconnect();
	});

	function say(said: ActorUserContent, attached: ImageUpload[] = []) {
		following = true;
		return actors.say(sessionId, said, attached);
	}

	/** Resolves the session: the user is done with it. */
	function resolve(): Promise<unknown> {
		return actors.resolve(sessionId);
	}

	async function send() {
		const text = draft;
		const attached = images;
		draft = '';
		images = [];
		try {
			await say({ kind: 'text', text }, attached);
		} catch (error) {
			draft = text;
			images = attached;
			toasts.error(error);
		}
	}

	// Fetch the images of each message that has some, once.
	$effect(() => {
		for (const message of messages)
			if (message.attachment_count > 0 && !actors.attachments[message.id])
				untrack(() => actors.loadAttachments(message)).catch(() => {});
	});

	function placeholder(): string {
		if (!actor) return 'This session’s actor is no longer offered.';
		if (!online) return `${actor.title} is offline.`;
		return 'Say what to do next…';
	}

	/** Whether message `index` is a tool or thinking still going on: the session works and its actor hasn't said it ended. */
	const running = (index: number) => stillRunning(session?.status, messages[index].content);

	/** Whether any tool or thinking is going on now. */
	const anyRunning = $derived(messages.some((_, index) => running(index)));

	/** Ticks each second while a tool or thinking goes on, so its time counts up. */
	let now = $state(nowSeconds());
	$effect(() => {
		if (!anyRunning) return;
		now = nowSeconds();
		const timer = setInterval(() => (now = nowSeconds()), 1000);
		return () => clearInterval(timer);
	});

	/**
	 * How long the tool or thinking at `index` took, in whole seconds: until
	 * its actor said it ended, counting up while it runs. One whose actor
	 * never said (an older one, or one the session stopped in) is taken to run
	 * until the next message, and is unknown if it was the last.
	 */
	function duration(index: number): string | null {
		const said = messages[index].content;
		const start = messages[index].sent_at;
		if (said.sender === 'actor' && 'ended_at' in said && said.ended_at !== null)
			return elapsed(said.ended_at - start);
		if (running(index)) return elapsed(Math.max(0, now - start));
		const next = messages[index + 1];
		return next ? elapsed(next.sent_at - start) : null;
	}

	/** Whether message `index` is a one-line row: a tool the actor ran, or its folded thinking. */
	const isLine = (index: number) =>
		['toolUse', 'thinking'].includes(messages[index]?.content.kind ?? '');

	/** How far `index` sits from what came before: wide on a change of speaker, none between one-line rows. */
	function spacing(index: number): string | false {
		if (index === 0) return false;
		if (messages[index].content.sender !== messages[index - 1].content.sender) return 'mt-6';
		if (isLine(index) && isLine(index - 1)) return 'mt-0';
		return 'mt-3';
	}

	/** Whether request `message` is settled: the user responded, or the session is no longer going on. */
	const answered = (message: ActorSessionMessage) =>
		message.answered || (session !== null && !isGoing(session));
</script>

<PageTitle title={`${session?.title ?? 'Session'} · Actors`} />

<!-- Fill the content area (over its bottom padding) and scroll inside, so
     the composer stays put. -->
<div class="-mb-12 flex h-[calc(100%+var(--s8))]">
	<div class="relative flex min-w-0 flex-1 flex-col">
		<div
			class="min-h-0 flex-1 [scrollbar-width:none] overflow-y-auto pb-6 [overflow-anchor:none]"
			style:padding-bottom={composerHeight ? `calc(${composerHeight}px + var(--s6))` : undefined}
			bind:this={scroller}
			{onscroll}
		>
			<div class="px-(--page-inset)" bind:this={content}>
				{#if missing}
					<PageHeader title="Actors" />
					<EmptyState
						message="Session not found"
						actionLabel="Back to actors"
						onaction={() => page.goto(routes.actors(), { replaceState: true })}
					/>
				{:else if session}
					<div class="sticky top-0 z-10">
						<SessionHeader {session} {actor}>
							{#snippet actions()}
								{#if session.status !== 'resolved'}
									<Button
										variant="secondary"
										onclick={() =>
											isGoing(session)
												? (resolving = true)
												: resolve().catch((error) => toasts.error(error))}>Resolve</Button
									>
								{/if}
							{/snippet}
						</SessionHeader>
					</div>

					<ol class="mx-auto flex w-full max-w-190 flex-col pt-3">
						{#each messages as message, index (message.id)}
							{@const said = message.content}
							<li
								class={[
									'flex min-w-0 flex-col',
									said.sender === 'user' && 'items-end',
									spacing(index)
								]}
							>
								{#if said.sender === 'user'}
									{#if message.attachment_count > 0}
										<SentImages
											count={message.attachment_count}
											images={actors.attachments[message.id]}
										/>
									{/if}
									<p
										class="max-w-[70%] min-w-0 rounded-md bg-accent-soft px-3 py-1.75 wrap-anywhere whitespace-pre-wrap max-sm:max-w-full"
									>
										{userText(said)}
									</p>
								{:else}
									{#if said.kind === 'text'}
										<div class="min-w-0 text-md">
											<Markdown source={said.text} diagrams />
										</div>
									{:else if said.kind === 'thinking'}
										<details class="group min-w-0">
											<summary
												class="flex h-6 min-w-0 cursor-pointer items-center gap-1.5 rounded-sm text-xs text-fg-subtle select-none hover:text-fg-muted focus-visible:shadow-focus focus-visible:outline-none [&::-webkit-details-marker]:hidden"
											>
												<!-- The chevron leads, in the icon's place, as every row that opens does in a conversation. -->
												<ChevronRight
													class="flex-none transition-transform group-open:rotate-90"
													size={12}
													strokeWidth={1.75}
													aria-hidden="true"
												/>
												{#if running(index)}<span class="sr-only">Running: </span>{/if}
												<Shimmer class="flex-none font-medium text-fg-muted" active={running(index)}
													>Thinking</Shimmer
												>
												{#if duration(index)}
													<span class="flex-none tabular-nums">{duration(index)}</span>
												{/if}
											</summary>
											<div
												class="mt-1 mb-2 ml-1.25 border-l-2 border-border pl-3 text-sm text-fg-muted"
											>
												<Markdown source={said.text} />
											</div>
										</details>
									{:else if said.kind === 'toolUse'}
										{@const ToolIcon = toolIcon(said.category)}
										<p class="flex h-6 min-w-0 items-center gap-1.5 text-xs text-fg-subtle">
											<ToolIcon class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
											{#if running(index)}<span class="sr-only">Running: </span>{/if}
											<Shimmer class="flex min-w-0 items-center gap-1.5" active={running(index)}>
												<span class="flex-none font-medium text-fg-muted">{said.tool}</span>
												<span class="min-w-0 truncate font-mono">{said.summary}</span>
											</Shimmer>
											<!-- Beside what it timed, not across the column. -->
											{#if duration(index)}
												<span class="flex-none tabular-nums">{duration(index)}</span>
											{/if}
										</p>
									{:else if said.kind === 'question'}
										<QuestionCard
											draftKey="actorSession.{sessionId}.question.{message.id}"
											questions={said.questions}
											answered={answered(message)}
											answers={responses.get(message.id) ?? null}
											onanswer={(answers) =>
												say({ kind: 'answers', request_id: message.id, answers })}
										/>
									{:else if said.kind === 'permission'}
										<PermissionCard
											request={said}
											answered={answered(message)}
											ondecide={(decision, note) =>
												say({
													kind: 'permission',
													request_id: message.id,
													decision,
													message: note
												})}
										/>
									{:else}
										<PlanCard
											plan={said.plan}
											answered={answered(message)}
											ondecide={(approved, feedback) =>
												say({ kind: 'plan', request_id: message.id, approved, feedback })}
										/>
									{/if}
									{#if message.attachment_count > 0}
										<SentImages
											count={message.attachment_count}
											images={actors.attachments[message.id]}
											place="under"
										/>
									{/if}
								{/if}
							</li>
						{/each}
					</ol>

					{#if session.status === 'working'}
						<WorkingLine />
					{/if}
				{/if}
			</div>
		</div>

		{#if session}
			<!-- Floats over the feed's foot, which scrolls under it. -->
			<div
				class="pointer-events-none absolute inset-x-0 bottom-0 pb-4"
				bind:clientHeight={composerHeight}
			>
				<div
					class="mx-auto w-full max-w-[calc(var(--spacing)*190+2*var(--page-inset))] px-(--page-inset)"
				>
					<SessionComposer
						bind:value={draft}
						bind:images
						disabled={!online}
						placeholder={placeholder()}
						onsend={send}
						onstop={isGoing(session) ? () => (stopping = true) : undefined}
						controls={session.context || plan.length || (actor && actor.modes.length > 1)
							? controls
							: undefined}
					/>
					<!-- The session's controls, inside the composer before Send, where the next message is decided. -->
					{#snippet controls()}
						{#if session.context || plan.length}
							<span class={['flex size-9 items-center justify-center', inGutter]}>
								<UsageRing context={session.context} {plan} />
							</span>
						{/if}
						{#if actor && actor.modes.length > 1}
							<ModePicker
								modes={actor.modes}
								mode={session.mode ?? actor.modes[0].identifier}
								disabled={!online}
								onpick={(mode) =>
									actors.setMode(session.id, mode).catch((error) => toasts.error(error))}
							/>
						{/if}
					{/snippet}
				</div>
			</div>

			<ConfirmDialog
				bind:open={stopping}
				title="Stop “{session.title}”?"
				description="The actor will stop what it’s doing. To pick this session back up, send a message."
				confirmLabel="Stop"
				onconfirm={() => actors.stop(session.id)}
			/>

			<ConfirmDialog
				bind:open={resolving}
				title="Resolve “{session.title}”?"
				description="The actor will stop what it’s doing. This session will be removed from your list. To pick it back up, send a message."
				confirmLabel="Resolve"
				onconfirm={resolve}
			/>
		{/if}
	</div>
</div>
