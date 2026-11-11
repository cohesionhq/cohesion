<!--
	An agent's conversation, oldest first: the user's messages as bubbles on
	the right, the agent's replies as bare Markdown across the column, as an
	actor's text is in its transcript, code it ran as collapsed rows,
	each module of the script API it loaded as a "Learning about" row. A fork
	shows as what woke it, a line opening in place onto the steps it took, their
	count at the line's trailing edge rolling as a live run adds to it
	(`collapseRuns`, by the `run_part` the daemon gives each message), with what
	it says to the user in the transcript as usual, under a caption naming what
	woke it when something else came between. Questions it asked with answers to pick from as
	cards, and each memory the agent remembered or forgot (a line that opens
	the memory beside it), and any kind of
	message this feed has no view for, as a quiet line of the message's
	excerpt. Images the user attached show above their message's text. A
	forward shows "Forwarded" above a link to the item, then the user's note,
	if any, as its bubble; a skill the user ran shows as a chip opening its
	prompt, then the note. A skill the agent ran itself is a
	"Using skill" row. The
	user can react to the agent's text messages with an emoji from a bar
	opened by resting on the react icon under the text, and the agent
	to the user's messages; a reaction sits under the message it answers, at the edge nearer the screen's.
	A reply of the agent's shows the message it answers above it, quoted in
	a muted line, its excerpt, that scrolls to that message. A reaction is
	recorded only by its badge under the message, wherever in the
	conversation the agent reacted.
	Three or more status lines in a row (forks' rows, quiet lines and memories) fold into one "5 events" line that
	opens onto them in place, so they never stand between two messages as a
	wall. Messages that arrive while the feed is shown rise in; the history it
	opens with and older pages loaded above stay put.
-->
<script lang="ts">
	import BookOpen from '@lucide/svelte/icons/book-open';
	import CheckSquare from '@lucide/svelte/icons/square-check';
	import FoldVertical from '@lucide/svelte/icons/fold-vertical';
	import Info from '@lucide/svelte/icons/info';
	import ListChecks from '@lucide/svelte/icons/list-checks';
	import SquareTerminal from '@lucide/svelte/icons/square-terminal';
	import Forward from '@lucide/svelte/icons/forward';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import CircleAlert from '@lucide/svelte/icons/circle-alert';
	import SmilePlus from '@lucide/svelte/icons/smile-plus';
	import Zap from '@lucide/svelte/icons/zap';
	import WandSparkles from '@lucide/svelte/icons/wand-sparkles';
	import AlarmClock from '@lucide/svelte/icons/alarm-clock';
	import { tick, type Component } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { fly } from 'svelte/transition';
	import {
		type AgentChoice,
		type AgentMessageContent,
		type IntelligenceAgent,
		type Attachment
	} from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { messageLine, type AgentMessage } from '$lib/stores/agents.svelte';
	import type { AgentWake } from '$lib/protocol/types';
	import Markdown from '$lib/ui/Markdown.svelte';
	import { motion } from '$lib/ui/motion';
	import Popover from '$lib/ui/Popover.svelte';
	import RollingText from '$lib/ui/RollingText.svelte';
	import AgentAsk from './AgentAsk.svelte';
	import ChoiceCard from './ChoiceCard.svelte';
	import CodeExecution from './CodeExecution.svelte';
	import ForwardChip from './ForwardChip.svelte';
	import MemoryPopover from './MemoryPopover.svelte';
	import SkillChip from './SkillChip.svelte';
	import { collapseRuns } from './runs';
	import ToolRow from './ToolRow.svelte';
	import SentImages from '$lib/ui/SentImages.svelte';

	interface Props {
		agent: IntelligenceAgent;
		messages: AgentMessage[];
		/** The images fetched so far, by message id. */
		attachments: Record<number, Attachment[]>;
		/** Sets the user's reaction to one of the agent's messages; null clears it. */
		onreact: (message: AgentMessage, reaction: string | null) => void;
		onpick: (message: AgentMessage, choice: AgentChoice, option: number) => Promise<unknown>;
	}

	let { agent, messages, attachments, onreact, onpick }: Props = $props();

	type AgentMemory = Extract<AgentMessageContent, { kind: 'memory' }>;

	/** The message whose reaction picker is open. */
	let reacting = $state<number | null>(null);

	/** Whether the user can react to `message`: one of the agent's saved text messages. */
	function reactable(message: AgentMessage): boolean {
		return message.id > 0 && message.message_content.kind === 'text' && message.sender !== 'user';
	}

	/** The kinds this feed has a view of its own for; any other is a quiet line. */
	const shaped = new Set([
		'text',
		'forward',
		'skill',
		'usedSkill',
		'choice',
		'codeExecution',
		'documentation',
		'error',
		'proposal',
		'askedAgent',
		'askedByAgent'
	]);

	/** The transcript with each fork's steps gathered under what woke it. */
	const collapsed = $derived(collapseRuns(messages));
	const entries = $derived(collapsed.shown);

	/** The forks whose steps are open, by run, kept as refetches replace the messages. */
	const opened = new SvelteSet<number>();

	/** Whether `message` is a question between this agent and another. */
	function asks(message: AgentMessage): boolean {
		const kind = message.message_content.kind;
		return kind === 'askedAgent' || kind === 'askedByAgent';
	}

	/**
	 * Whether `message` is a quiet one-line entry, its excerpt (a memory, or
	 * a kind this feed doesn't know), which belongs to the turn or run that
	 * posted it.
	 */
	function quiet(message: AgentMessage): boolean {
		return !shaped.has(message.message_content.kind);
	}

	// What the last render showed, so only messages newer than all of it
	// animate: not older pages prepended, and not the saved copy of the
	// user's message that replaces its stand-in, already on screen.
	let shown = new Set<number>();
	let newest = 0;
	const arriving = $derived.by(() => {
		const standInSaved = [...shown].some((id) => id < 0 && !messages.some((m) => m.id === id));
		const fresh = new Set(
			messages
				.filter((m) => !shown.has(m.id) && (m.id < 0 || m.id > newest))
				.filter((m) => !(standInSaved && m.sender === 'user'))
				.map((m) => m.id)
		);
		shown = new Set(messages.map((m) => m.id));
		newest = Math.max(newest, ...messages.map((m) => m.id));
		return fresh;
	});

	/** Scrolls the feed to the message `id`, which a reply quotes, opening the fork it's a step of. */
	async function reveal(id: number) {
		const target = messages.find((m) => m.id === id);
		if (target?.run_part === 'step' && target.run_id !== null) {
			opened.add(target.run_id);
			await tick();
		}
		document
			.getElementById(`agent-message-${id}`)
			?.scrollIntoView({ block: 'center', behavior: motion.duration ? 'smooth' : 'auto' });
	}

	/** Why the agent woke, as its wake line says it. */
	function wokeTitle(by: AgentWake): string {
		switch (by.kind) {
			case 'cue':
				return `Cue “${by.cue_name}”`;
			case 'actorSession':
				return `Actor session “${by.title}” ${vocabulary.actorSessionStop(by.stop)}`;
			case 'assignedTask':
				return `“${by.title}” was assigned to you`;
		}
	}

	/** What woke the run `message` belongs to, in one line: a trigger's own, or what it knows of it. */
	function runTitle(message: AgentMessage): string {
		const content = message.message_content;
		if (content.kind === 'woke') {
			const woke = wokeTitle(content.by);
			return message.run_description ? `${woke} · ${message.run_description}` : woke;
		}
		if (message.run_part === 'trigger') return message.excerpt;
		return message.run_description ?? (message.run_id === null ? 'Job' : 'An earlier run');
	}

	/** The icon of a fork's row: what woke it, or a bolt when that isn't a wake. */
	function runIcon(message: AgentMessage): Component {
		const content = message.message_content;
		if (content.kind !== 'woke') return Zap;
		switch (content.by.kind) {
			case 'cue':
				return AlarmClock;
			case 'actorSession':
				return SquareTerminal;
			case 'assignedTask':
				return CheckSquare;
		}
	}

	/** The icon of a quiet line: what it records. A memory draws its own, in `MemoryPopover`. */
	function quietIcon(message: AgentMessage): Component {
		switch (message.message_content.kind) {
			case 'rebuiltContext':
				return FoldVertical;
			case 'agenda':
				return ListChecks;
			default:
				return Info;
		}
	}

	/** Whether `message` draws as a one-line status: a fork's row or a quiet line. */
	function statusLine(message: AgentMessage): boolean {
		if (quiet(message)) return true;
		if (message.run_id === null || message.run_part === 'surfaced') return false;
		const steps = collapsed.steps.get(message.run_id) ?? [];
		return steps.length > 0 || message.message_content.kind !== 'askedByAgent';
	}

	/** Who sent `message`, telling forks apart. */
	function source(message: AgentMessage): string {
		return message.run_id === null ? message.sender : `run ${message.run_id}`;
	}

	/**
	 * Whether `index` opens a new block: a change of speaker, or a job's
	 * message, each of which stands on its own. Blocks are spaced apart;
	 * the messages inside one (a reply and the code it ran, consecutive
	 * user messages) sit close together. A status or memory line belongs to
	 * the turn or job that posted it: it opens that block when it comes first, and
	 * what follows it from the same sender stays in its block. A job's
	 * messages, posted one after another, are one block.
	 */
	function startsBlock(index: number): boolean {
		if (index === 0) return false;
		const message = entries[index];
		const previous = entries[index - 1];
		if (quiet(previous) && source(previous) === source(message)) return false;
		return source(message) !== source(previous);
	}

	/**
	 * Whether a status line sits right above `index`, one line of a stack. A
	 * fork open onto its steps isn't one: what follows keeps its distance from them.
	 */
	function followsStatus(index: number): boolean {
		if (index === 0) return false;
		const previous = entries[index - 1];
		return statusLine(previous) && !(previous.run_id !== null && opened.has(previous.run_id));
	}

	/** Cancels the list's gap, so status lines stack with none, as an actor transcript's tool rows do. */
	const stacked = '-mt-2';

	/** The space above `index` beyond the list's gap: none in a stack of status lines, more for a new block. */
	function blockGap(index: number): string {
		if (statusLine(entries[index]) && followsStatus(index)) {
			return stacked;
		}
		return startsBlock(index) ? 'mt-3' : '';
	}

	/** The entries the user unfolded from a run of status lines, kept as refetches replace the messages. */
	const unfolded = new SvelteSet<number>();

	/**
	 * Runs of three or more status lines in a row (with the reaction lines between them), folded
	 * into one line that opens in place: by the index of the run's first entry, how many lines it
	 * holds and the index of its last entry. A run the user opened stays open.
	 */
	const folds = $derived.by(() => {
		const runs: Record<number, { end: number; count: number }> = {};
		const folding = (index: number) => {
			const message = entries[index];
			return statusLine(message) && !(message.run_id !== null && opened.has(message.run_id));
		};
		let start = 0;
		while (start < entries.length) {
			if (!folding(start)) {
				start++;
				continue;
			}
			let end = start;
			let count = 1;
			while (end + 1 < entries.length && folding(end + 1)) {
				end++;
				count++;
			}
			const ids = entries.slice(start, end + 1).map((m) => m.id);
			if (count >= 3 && !ids.some((id) => unfolded.has(id))) runs[start] = { end, count };
			start = end + 1;
		}
		return runs;
	});

	/** The indexes a folded run hides: all of it but its first, which draws the folded line. */
	const folded = $derived.by(() => {
		const hidden: Record<number, true> = {};
		for (const [start, { end }] of Object.entries(folds)) {
			for (let index = Number(start) + 1; index <= end; index++) hidden[index] = true;
		}
		return hidden;
	});

	/** Opens the folded run starting at `start`. */
	function unfold(start: number) {
		const run = folds[start];
		if (!run) return;
		for (let index = start; index <= run.end; index++) unfolded.add(entries[index].id);
	}

	/**
	 * Whether `index` needs a caption saying what woke the run it came from:
	 * what a fork or a job told the user, with something other than the same
	 * run above it.
	 */
	function opensRun(index: number): boolean {
		const message = entries[index];
		const told =
			message.run_part === 'surfaced' || (message.sender === 'backgroundAgent' && !asks(message));
		return told && (index === 0 || source(entries[index - 1]) !== source(message));
	}
</script>

<!-- The emoji to react with, one click each; the current reaction is
     marked, and picking it again clears it. -->
{#snippet choices(message: AgentMessage)}
	<div class="flex gap-0.5" role="group" aria-label="Reactions">
		{#each vocabulary.value.agent_reactions as reaction (reaction)}
			<button
				class={[
					'inline-grid size-8 cursor-pointer place-items-center rounded-full text-lg leading-none transition-transform hover:bg-bg-hover focus-visible:bg-bg-hover focus-visible:outline-none motion-safe:hover:scale-125',
					message.reaction === reaction && 'bg-accent-soft'
				]}
				type="button"
				aria-pressed={message.reaction === reaction}
				onclick={(event) => {
					reacting = null;
					// Picking closes the bar, which focus inside it would hold open.
					event.currentTarget.blur();
					onreact(message, message.reaction === reaction ? null : reaction);
				}}>{reaction}</button
			>
		{/each}
	</div>
{/snippet}

<!-- Without hover, a button opens the same emoji in a popover. -->
{#snippet react(message: AgentMessage)}
	<Popover
		bind:open={() => reacting === message.id, (open) => (reacting = open ? message.id : null)}
		side="top"
		align="start"
	>
		{#snippet trigger(props)}
			<button
				class="inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-full text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg"
				type="button"
				{...props}
				aria-label="React"
			>
				<SmilePlus size={16} strokeWidth={1.75} />
			</button>
		{/snippet}
		{@render choices(message)}
	</Popover>
{/snippet}

<!-- The reaction under a message, the agent's on the user's bubble or the user's on the agent's text. -->
{#snippet reaction(message: AgentMessage)}
	{#if message.reaction}
		<!-- One chip for every reaction, as a thread draws them: an outlined capsule under the
		     message, on its side. The agent's, under the user's bubble, is only shown; the user's,
		     under the agent's text, clears on a click. -->
		{@const chip =
			'mt-1 inline-flex h-6 items-center rounded-full border border-border bg-bg px-2 text-xs leading-none'}
		{#if message.sender === 'user'}
			<span class={[chip, 'self-end']}>{message.reaction}</span>
		{:else}
			<button
				class={[
					chip,
					'cursor-pointer self-start transition-colors hover:border-border-strong hover:bg-bg-hover'
				]}
				type="button"
				aria-label="Remove your reaction {message.reaction}"
				onclick={() => onreact(message, null)}>{message.reaction}</button
			>
		{/if}
	{/if}
{/snippet}

<!-- The message a reply answers, in one muted line behind a bar, like a
     quote in a text thread; one not loaded (or gone) reads as an earlier
     message. Clicking it scrolls to the message. -->
{#snippet quote(id: number)}
	{@const quoted = messages.find((m) => m.id === id)}
	{@const line =
		'mb-1 max-w-full min-w-0 self-start truncate border-l-2 border-border pl-2 text-left text-xs leading-5 text-fg-subtle'}
	{#if quoted}
		<button class={[line, 'cursor-pointer hover:text-fg']} type="button" onclick={() => reveal(id)}
			>{messageLine(quoted)}</button
		>
	{:else}
		<p class={line}>An earlier message</p>
	{/if}
{/snippet}

<!-- A quiet line: its icon, then its words. -->
{#snippet quietLine(message: AgentMessage)}
	{@const Icon = quietIcon(message)}
	<p class="flex max-w-full min-w-0 items-center gap-1.5 text-xs leading-6 text-fg-subtle">
		<Icon class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
		<span class="min-w-0 truncate">{messageLine(message)}</span>
	</p>
{/snippet}

{#snippet images(message: AgentMessage, place: 'over' | 'under')}
	<SentImages count={message.attachment_count} images={attachments[message.id]} {place} />
{/snippet}

<!-- The react control: on hover, a disc under the text at its left edge, after
     the reaction if there is one, laid over the gap so messages stay close.
     Its padding meets the text, so the pointer never leaves the message on the
     way down, and until the message is hovered it takes no pointer events, so
     it never sits over the next message's first words. Resting the pointer on
     it opens the reaction bar there, so passing over it does nothing and
     selecting the text never brings the bar up; clicking or focusing it opens
     the bar at once. Its parent is the text's box, positioned. -->
{#snippet tools(message: AgentMessage)}
	<span
		class={[
			'group/react pointer-events-none absolute top-full z-1 pt-1 group-focus-within:pointer-events-auto group-hover:pointer-events-auto touch:hidden',
			message.reaction ? 'left-9' : 'left-0'
		]}
	>
		<button
			class="pointer-events-none grid size-6 cursor-pointer place-items-center rounded-full bg-n2 text-fg-subtle opacity-0 ring-2 ring-bg transition-opacity group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100 hover:text-fg"
			type="button"
			aria-label="React"
		>
			<SmilePlus size={14} strokeWidth={1.75} />
		</button>
		<span
			class={[
				'overlay-surface invisible absolute top-1/2 left-0 -translate-y-1/2 rounded-full p-1 opacity-0 transition-[opacity,visibility] delay-0',
				'group-hover/react:visible group-hover/react:opacity-100 group-hover/react:delay-400',
				'group-focus-within/react:visible group-focus-within/react:opacity-100 group-focus-within/react:delay-0'
			]}
		>
			{@render choices(message)}
		</span>
	</span>
{/snippet}

<!-- A script, a loaded module, a note, a question between agents, a failure: what the agent did, in a row. -->
{#snippet work(message: AgentMessage)}
	{@const content = message.message_content}
	<div class="min-w-0">
		{#if content.kind === 'choice'}
			<ChoiceCard choice={content} onpick={(option) => onpick(message, content, option)} />
		{:else if content.kind === 'codeExecution'}
			<CodeExecution execution={content} />
		{:else if content.kind === 'documentation'}
			<ToolRow title={message.excerpt} icon={BookOpen} />
		{:else if content.kind === 'usedSkill'}
			<ToolRow title={message.excerpt} icon={WandSparkles} />
		{:else if content.kind === 'note'}
			<ToolRow title={messageLine(message)}>
				<p class="text-sm wrap-anywhere whitespace-pre-wrap text-fg-muted">
					{content.note}
				</p>
			</ToolRow>
		{:else if content.kind === 'askedAgent' || content.kind === 'askedByAgent'}
			<AgentAsk ask={content} title={message.excerpt} />
		{:else if content.kind === 'error'}
			<p class="flex items-start gap-1.5 text-sm text-danger">
				<CircleAlert class="mt-0.5 flex-none" size={14} strokeWidth={1.75} aria-label="Failed" />
				<span class="min-w-0 wrap-anywhere whitespace-pre-wrap">{content.error}</span>
			</p>
		{/if}
	</div>
{/snippet}

<!-- One line of a memory, which opens the memory beside it. -->
{#snippet memory(message: AgentMessage, content: AgentMemory)}
	<MemoryPopover
		{agent}
		text={content.memory}
		change={content.change}
		line={messageLine(message)}
	/>
{/snippet}

<!-- A fork in the transcript: what woke it, opening in place onto the steps it
     took. `anchor` is the trigger, or the first step when the trigger isn't
     loaded. A fork that took no steps has nothing to open but another agent's
     question, in its usual row. -->
{#snippet run(anchor: AgentMessage, runId: number)}
	{@const content = anchor.message_content}
	{@const steps = collapsed.steps.get(runId) ?? []}
	{@const Icon = runIcon(anchor)}
	{#if steps.length > 0}
		<details
			class="group/run min-w-0"
			bind:open={
				() => opened.has(runId), (open) => (open ? opened.add(runId) : opened.delete(runId))
			}
		>
			<summary
				class="flex cursor-pointer items-center gap-1.5 text-xs leading-6 text-fg-subtle select-none hover:text-fg-muted [&::-webkit-details-marker]:hidden"
			>
				<Icon class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
				<span class="min-w-0 truncate">{runTitle(anchor)}</span>
				<!-- Beside what woke it, as an actor transcript's rows place their duration. -->
				<!-- Pulled back over the row's gap, so "· 5 steps" follows the title at a word space. -->
				<span class="-ml-1.5 flex-none tabular-nums">
					<span aria-hidden="true">&nbsp;·</span>
					<RollingText text={String(steps.length)} reserve={1} />
					{steps.length === 1 ? 'step' : 'steps'}
				</span>
				<!-- A hairline to the column's edge, so what woke a fork reads as a divider between turns. -->
				<span class="h-px min-w-6 flex-1 bg-border" aria-hidden="true"></span>
			</summary>
			<ol class="mt-1 ml-1.5 flex flex-col gap-2 border-l-2 border-border pl-3">
				{#if content.kind === 'askedByAgent'}
					<li>{@render work(anchor)}</li>
				{/if}
				{#each steps as step (step.id)}
					{@const stepContent = step.message_content}
					<li
						id="agent-message-{step.id}"
						in:fly={{ y: 8, duration: arriving.has(step.id) ? motion.duration : 0 }}
					>
						{#if stepContent.kind === 'memory'}
							{@render memory(step, stepContent)}
						{:else if quiet(step) && stepContent.kind !== 'note'}
							{@render quietLine(step)}
						{:else}
							{@render work(step)}
						{/if}
					</li>
				{/each}
			</ol>
		</details>
	{:else if content.kind === 'askedByAgent'}
		{@render work(anchor)}
	{:else}
		<p class="flex items-center gap-1.5 text-xs leading-6 text-fg-subtle">
			<Icon class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
			<span class="min-w-0 truncate">{runTitle(anchor)}</span>
			<span class="h-px min-w-6 flex-1 bg-border" aria-hidden="true"></span>
		</p>
	{/if}
{/snippet}

<ol class="mx-auto flex w-full max-w-190 flex-col gap-2 pt-3">
	{#each entries as message, index (message.id)}
		{@const user = message.sender === 'user'}
		{@const content = message.message_content}
		{@const fold = folds[index]}
		{#if !folded[index]}
			{#if fold}
				<!-- A run of status lines, folded into one quiet line that opens onto them. -->
				<li class={['flex', blockGap(index)]}>
					<button
						class="flex max-w-full min-w-0 cursor-pointer items-center gap-1.5 text-left text-xs leading-6 text-fg-subtle hover:text-fg-muted"
						type="button"
						aria-expanded="false"
						onclick={() => unfold(index)}
					>
						<ChevronRight class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
						{fold.count} events
					</button>
				</li>
			{:else}
				<li
					id={message.run_part === 'step' ? undefined : `agent-message-${message.id}`}
					class={['group flex items-start', blockGap(index)]}
					in:fly={{ y: 8, duration: arriving.has(message.id) ? motion.duration : 0 }}
				>
					<div class={['flex min-w-0 flex-1 flex-col', user && 'items-end']}>
						{#if message.run_id !== null && message.run_part !== 'surfaced'}
							{@render run(message, message.run_id)}
						{:else if content.kind === 'memory'}
							{@render memory(message, content)}
						{:else if quiet(message)}
							{@render quietLine(message)}
						{:else if (content.kind === 'text' || content.kind === 'forward' || content.kind === 'skill') && user}
							{@const text = content.kind === 'text' ? content.text : content.note}
							{#if content.kind === 'skill'}
								<div class={['flex max-w-[70%] justify-end max-sm:max-w-full', text && 'mb-1']}>
									<SkillChip name={content.name} prompt={content.prompt} />
								</div>
							{:else if content.kind === 'forward'}
								<p class="mb-1 flex items-center gap-1.5 text-xs text-fg-subtle">
									<Forward class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
									Forwarded
								</p>
								<div class={['flex max-w-[70%] justify-end max-sm:max-w-full', text && 'mb-1']}>
									<ForwardChip item={content.item} />
								</div>
							{/if}
							{#if message.attachment_count > 0}
								{@render images(message, 'over')}
							{/if}
							{#if text}
								<p
									class="max-w-[70%] min-w-0 rounded-md bg-accent-soft px-3 py-1.75 wrap-anywhere whitespace-pre-wrap max-sm:max-w-full"
								>
									{text}
								</p>
							{/if}
							{@render reaction(message)}
						{:else}
							{#if opensRun(index)}
								<!-- What woke the run, once above what it says; it scrolls there when it's loaded. -->
								{@const trigger =
									message.run_id === null ? undefined : collapsed.triggers.get(message.run_id)}
								{@const caption =
									'mb-1 flex w-full items-center gap-1.5 text-left text-xs text-fg-subtle'}
								{@const WhyIcon = runIcon(trigger ?? message)}
								{#snippet why()}
									<!-- The same icon as the run's own line, so one cue reads the same in both places. -->
									<WhyIcon class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
									<span class="min-w-0 truncate">{runTitle(trigger ?? message)}</span>
									<!-- The same divider as the run's own line. -->
									<span class="h-px min-w-6 flex-1 bg-border" aria-hidden="true"></span>
								{/snippet}
								{#if trigger}
									<button
										class={[caption, 'cursor-pointer hover:text-fg']}
										type="button"
										onclick={() => reveal(trigger.id)}>{@render why()}</button
									>
								{:else}
									<div class={caption}>{@render why()}</div>
								{/if}
							{/if}
							{#if message.reply_to !== null}
								{@render quote(message.reply_to)}
							{/if}
							{#if content.kind === 'text'}
								<!-- Bare, like an actor's text in its transcript: the column is the reply's shape. -->
								<div class="relative min-w-0 text-md">
									{#if reactable(message)}
										{@render tools(message)}
									{/if}
									<Markdown source={content.text} diagrams />
								</div>
							{:else}
								{@render work(message)}
							{/if}
							<!-- The images the agent showed with its text, below the text. -->
							{#if message.attachment_count > 0}
								{@render images(message, 'under')}
							{/if}
							{#if reactable(message) && !message.reaction}
								<span class="-ml-1.5 hidden self-start touch:inline-flex"
									>{@render react(message)}</span
								>
							{/if}
							{@render reaction(message)}
						{/if}
					</div>
				</li>
			{/if}
		{/if}
	{/each}
</ol>
