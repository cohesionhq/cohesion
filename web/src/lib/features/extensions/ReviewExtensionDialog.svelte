<!--
	The review before installing, from a registry or a file, laid out like an
	OAuth consent screen: the extension's icon on the dialog's top edge, then what
	it will be able to do and when it runs. For a registry entry the
	daemon downloads the script and reads its real manifest, and this marks
	where it differs from what the registry listed; a file arrives already
	read. The code starts collapsed. Install sends back exactly the script
	shown, so what's installed is what was read.
-->
<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type {
		ExtensionManifest,
		ExtensionPreview,
		HookKind,
		RegistryResult
	} from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { extensions, hookTitle, sortHooks, sortPermissions } from '$lib/stores/extensions.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Editor from '$lib/ui/Editor.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import GrantRow from './GrantRow.svelte';
	import { grantNoun, PERMISSION_ICONS } from './grants';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	interface Props {
		open?: boolean;
		/** The registry's entry: where the script is, and what the registry claims it is. */
		result?: RegistryResult | null;
		/** A script from a file or a paste, already read by the daemon. */
		file?: { name: string | null; preview: ExtensionPreview } | null;
	}

	let { open = $bindable(false), result = null, file = null }: Props = $props();

	let preview = $state<ExtensionPreview | null>(null);
	let error = $state<string | null>(null);
	let installError = $state<string | null>(null);
	let busy = $state(false);
	/** A `text/plain` copy of the shown script for "Open in new tab"; revoked when the preview goes. */
	let scriptUrl = $state<string | null>(null);
	let generation = 0;

	$effect(() => {
		if (!open) return;
		if (result) {
			const url = result.source_url;
			untrack(() => void load(url));
		} else if (file) preview = file.preview;
		else return;
		return () => generation++;
	});

	function reset() {
		preview = null;
		error = null;
		installError = null;
	}

	$effect(() => {
		if (!preview) return;
		const url = URL.createObjectURL(new Blob([preview.script], { type: 'text/plain' }));
		scriptUrl = url;
		return () => {
			URL.revokeObjectURL(url);
			scriptUrl = null;
		};
	});

	async function load(source_url: string) {
		const current = ++generation;
		try {
			const found = await extensions.preview(source_url);
			if (current === generation) preview = found;
		} catch (e) {
			if (current === generation) error = toProtocolError(e).message;
		}
	}

	// What to show before the script is read: the registry's listing, or the file.
	const manifest = $derived(preview?.manifest ?? result?.manifest ?? file?.preview.manifest);
	/** Whether this replaces an installed extension, known before the script is read. */
	const updating = $derived(
		manifest !== undefined &&
			extensions.list.value.some((e) => e.script_identifier === manifest.identifier)
	);

	const FIELDS = [
		{ key: 'name', label: 'Name' },
		{ key: 'version', label: 'Version' },
		{ key: 'author', label: 'Author' },
		{ key: 'identifier', label: 'Identifier' },
		{ key: 'description', label: 'Description' }
	] as const satisfies { key: keyof ExtensionManifest; label: string }[];

	/** Entries of `real` that `listed` doesn't have. */
	function missingFrom<T>(real: T[], listed: T[]): Set<T> {
		return new Set(real.filter((x) => !listed.includes(x)));
	}

	const NO_DIFFERENCES = {
		fields: new Set<string>(),
		undeclared: new Set<string>(),
		dropped: [] as string[],
		differs: false
	};

	// A file has no registry listing to compare against.
	const comparison = $derived.by(() => {
		if (!preview) return null;
		if (!result) return NO_DIFFERENCES;
		const listed = result.manifest;
		const real = preview.manifest;
		// `description` and `author` may be absent, which reads as empty.
		const fields = new Set(
			FIELDS.filter((f) => (listed[f.key] ?? '') !== (real[f.key] ?? '')).map((f) => f.key)
		);
		// Undeclared: asked for by the script, not listed by the registry.
		const undeclared = new Set<string>([
			...missingFrom(real.permissions, listed.permissions),
			...missingFrom(real.hooks, listed.hooks)
		]);
		// Listed by the registry but not asked for: harmless, but still a mismatch.
		const dropped = [
			...missingFrom(listed.permissions, real.permissions),
			...missingFrom(listed.hooks, real.hooks)
		];
		return {
			fields,
			undeclared,
			dropped,
			differs: fields.size > 0 || undeclared.size > 0 || dropped.length > 0
		};
	});

	async function install() {
		if (!preview) return;
		busy = true;
		installError = null;
		try {
			const { manifest, installed } = preview;
			await extensions.install(preview.script, installed !== null);
			open = false;
			if (installed !== null) {
				toasts.show(`Updated ${manifest.name} to ${manifest.version}.`, 'info');
			} else {
				// A new install starts with everything off: open its page to enable things.
				const added = extensions.list.value.find(
					(e) => e.script_identifier === manifest.identifier
				);
				const asks = grantNoun(manifest.permissions.length, manifest.hooks.length);
				toasts.show(
					manifest.permissions.length + manifest.hooks.length > 0
						? `Installed ${manifest.name}. ${manifest.name} is inactive until you enable its ${asks}.`
						: `Installed ${manifest.name}.`,
					'info'
				);
				if (added) void page.goto(routes.extension(added.id));
			}
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') installError = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet hero()}
	<IconTile icon={manifest?.icon ?? null} title={manifest?.name ?? ''} size={64} />
{/snippet}

<Dialog
	bind:open
	onclosed={reset}
	centered
	{hero}
	title={updating
		? `Update ${manifest?.name ?? 'extension'}`
		: `Add ${manifest?.name ?? 'extension'} to Cohesion`}
	description={manifest
		? [
				manifest.author && `By ${manifest.author}`,
				manifest.version && `Version ${manifest.version}`
			]
				.filter(Boolean)
				.join(' · ') || undefined
		: undefined}
>
	<div class="flex flex-col gap-4">
		{#if error !== null}
			<p class="text-xs whitespace-pre-wrap text-danger" role="alert">
				Couldn’t fetch the script: {error}
			</p>
		{:else if !preview || !comparison}
			<p class="text-center text-fg-subtle" role="status">Fetching the script…</p>
		{:else}
			{#if comparison.differs}
				<div
					class="flex items-start gap-2 rounded-md bg-warn/12 px-3 py-2 text-warn [&_svg]:mt-0.5 [&_svg]:flex-none"
					role="alert"
				>
					<TriangleAlert size={15} strokeWidth={1.75} aria-hidden="true" />
					<div class="flex flex-col gap-1">
						<p>The script differs from what the registry listed:</p>
						<ul class="flex flex-col text-xs">
							{#each FIELDS.filter((f) => comparison.fields.has(f.key)) as field (field.key)}
								<li class="wrap-anywhere">
									{field.label}: listed as “{result?.manifest[field.key] || '(nothing)'}”
								</li>
							{/each}
							{#if comparison.dropped.length > 0}
								<li>Listed, but the script doesn’t ask for: {comparison.dropped.join(', ')}</li>
							{/if}
						</ul>
					</div>
				</div>
			{/if}

			{#if preview.manifest.description}
				<p class="text-center text-fg-muted">{preview.manifest.description}</p>
			{/if}

			<section class="flex flex-col gap-2">
				<h3 class="text-xs font-medium text-fg-subtle">What it asks for</h3>
				<ul class="flex flex-col divide-y divide-border rounded-md border border-border">
					{#each sortPermissions(preview.manifest.permissions) as name (name)}
						<GrantRow
							icon={PERMISSION_ICONS[name]}
							title={vocabulary.permission(name).title}
							detail={vocabulary.permission(name).description}
							flag={comparison.undeclared.has(name) ? 'Not listed' : undefined}
						/>
					{:else}
						<li class="px-3 py-2.5 text-xs text-fg-subtle">None.</li>
					{/each}
				</ul>
			</section>

			{#if preview.manifest.hooks.length > 0}
				<section class="flex flex-col gap-2">
					<h3 class="text-xs font-medium text-fg-subtle">When it runs</h3>
					<ul class="flex flex-col divide-y divide-border rounded-md border border-border">
						{#each sortHooks(preview.manifest.hooks) as name (name)}
							<GrantRow
								title={hookTitle(name as HookKind)}
								flag={comparison.undeclared.has(name) ? 'Not listed' : undefined}
							/>
						{/each}
					</ul>
				</section>
			{/if}

			{#if preview.installed}
				<p class="text-center text-xs text-fg-subtle">
					Replaces version {preview.installed.version} and keeps the access you gave it.
				</p>
			{:else if preview.manifest.permissions.length > 0 || preview.manifest.hooks.length > 0}
				<p class="text-center text-xs text-fg-subtle">
					{preview.manifest.name} will be inactive until you enable its {grantNoun(
						preview.manifest.permissions.length,
						preview.manifest.hooks.length
					)} on its page.
				</p>
			{/if}

			<details class="group flex flex-col gap-1 border-t border-border pt-3">
				<summary
					class="flex cursor-pointer items-center gap-1.5 text-xs font-medium text-fg-subtle select-none hover:text-fg [&::-webkit-details-marker]:hidden"
				>
					<ChevronRight
						class="flex-none transition-transform group-open:rotate-90"
						size={14}
						strokeWidth={1.75}
					/>
					Details and script
					{#if scriptUrl}
						<a
							class="ml-auto font-normal"
							href={scriptUrl}
							target="_blank"
							rel="noopener noreferrer">Open in new tab</a
						>
					{/if}
				</summary>
				<dl class="mt-2 mb-1 grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1 text-xs">
					<dt class="text-fg-subtle">Identifier</dt>
					<dd class="font-mono wrap-anywhere">{preview.manifest.identifier}</dd>
					{#if result || file?.name}
						<dt class="text-fg-subtle">From</dt>
						<dd class="font-mono wrap-anywhere">{result?.source_url ?? file?.name}</dd>
					{/if}
				</dl>
				<!-- Focusable so the code scrolls from the keyboard; the editor itself is read-only. -->
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<div
					class="mt-1 max-h-[40vh] overflow-y-auto rounded-sm border border-border-strong p-2"
					role="region"
					aria-label="Extension script"
					tabindex="0"
				>
					<Editor
						value={preview.script}
						language="javascript"
						label="Extension script"
						readonly
						minHeight="0"
					/>
				</div>
			</details>
		{/if}
		{#if installError}
			<p class="text-xs whitespace-pre-wrap text-danger" role="alert">{installError}</p>
		{/if}
	</div>
	{#snippet footer()}
		<Button variant="primary" onclick={install} disabled={busy || !preview}>
			{#if updating}{busy ? 'Updating…' : 'Update'}{:else}{busy ? 'Installing…' : 'Install'}{/if}
		</Button>
		<Button class="border-border-strong!" onclick={() => (open = false)}>Cancel</Button>
	{/snippet}
</Dialog>
