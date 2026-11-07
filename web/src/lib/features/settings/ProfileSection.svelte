<!--
	Settings › Profile: who the user is, centred, with no rows. Their disc
	opens the file picker to add or change their photo, with a quiet button
	beside it to remove one. Their name below it is text that edits in place,
	saved when it loses focus or on Enter; Escape puts it back. Their bio below
	that edits the same way, with Shift+Enter for a new line, and keeps what
	was typed when it is refused, so a long one can be cut rather than lost. A
	picked image is scaled to `PHOTO_SIDE` and re-encoded as JPEG when larger,
	as a contact's photo is.
-->
<script lang="ts">
	import Camera from '@lucide/svelte/icons/camera';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { untrack } from 'svelte';
	import { profile } from '$lib/stores/settings/profile.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { imageFiles, PHOTO_SIDE, prepareImage } from '$lib/ui/images';
	import Photo from '$lib/ui/Photo.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	$effect(() => {
		untrack(() => void profile.load());
		return () => profile.release();
	});

	const BIO_PLACEHOLDER = 'What agents should know about you…';

	const current = $derived(profile.profile.value);

	const savedName = $derived(current?.name ?? '');
	// The typed name, reset to the saved one whenever that changes.
	let name = $derived(savedName);
	let savingName = $state(false);

	async function commitName() {
		if (name.trim() === savedName) {
			name = savedName;
			return;
		}
		savingName = true;
		try {
			await profile.setName(name);
		} catch (error) {
			name = savedName;
			toasts.error(error);
		} finally {
			savingName = false;
		}
	}

	const savedBio = $derived(current?.bio ?? '');
	// The typed bio, reset to the saved one whenever that changes.
	let bio = $derived(savedBio);
	let savingBio = $state(false);
	const bioHint = $props.id();

	async function commitBio() {
		if (bio.trim() === savedBio) {
			bio = savedBio;
			return;
		}
		savingBio = true;
		try {
			await profile.setBio(bio);
		} catch (error) {
			toasts.error(error);
		} finally {
			savingBio = false;
		}
	}

	let picker = $state<HTMLInputElement>();
	let removing = $state(false);

	async function onpick(event: Event & { currentTarget: HTMLInputElement }) {
		const [file] = imageFiles(event.currentTarget.files);
		event.currentTarget.value = '';
		if (!file) return;
		try {
			await profile.setPhoto(await prepareImage(file, PHOTO_SIDE));
		} catch (error) {
			toasts.error(error);
		}
	}
</script>

<ResourceState resource={profile.profile}>
	{#snippet loaded()}
		{#if current}
			<section class="mx-auto flex max-w-100 flex-col items-center gap-3 py-8" aria-label="Profile">
				<div class="relative">
					<button
						class="group relative flex cursor-pointer rounded-full focus-visible:shadow-focus focus-visible:outline-none"
						type="button"
						aria-label={current.photo.kind === 'image' ? 'Change photo' : 'Add photo'}
						onclick={() => picker?.click()}
					>
						<Photo photo={current.photo} name={current.name} size="xl" />
						<span
							class="absolute right-0 bottom-0 flex size-7 items-center justify-center rounded-full border border-border bg-bg text-fg-muted transition-colors group-hover:bg-bg-hover group-hover:text-fg"
						>
							<Camera size={14} strokeWidth={1.75} />
						</span>
					</button>
					{#if current.photo.kind === 'image'}
						<button
							class="absolute top-1/2 left-full ml-3 flex size-7.5 -translate-y-1/2 cursor-pointer items-center justify-center rounded-sm text-fg-subtle transition-colors hover:bg-danger/10 hover:text-danger focus-visible:shadow-focus focus-visible:outline-none"
							type="button"
							aria-label="Remove photo"
							onclick={() => (removing = true)}
						>
							<Trash2 size={15} strokeWidth={1.75} />
						</button>
					{/if}
				</div>
				<!-- The hidden copy sizes the field to its text, so it reads as a heading. -->
				<div class="relative max-w-full min-w-24">
					<span
						class="invisible block h-8 overflow-hidden px-2 text-lg font-semibold whitespace-pre"
						aria-hidden="true">{name || 'Name'}</span
					>
					<input
						class="absolute inset-0 w-full rounded-sm bg-transparent px-2 text-center text-lg font-semibold text-fg transition-colors placeholder:text-fg-subtle hover:bg-bg-hover focus:bg-transparent focus:shadow-focus focus:outline-none"
						type="text"
						placeholder="Name"
						aria-label="Name"
						autocomplete="name"
						spellcheck="false"
						readonly={savingName}
						bind:value={name}
						onchange={() => void commitName()}
						onkeydown={(event) => {
							if (event.key === 'Enter') event.currentTarget.blur();
							if (event.key === 'Escape') {
								name = savedName;
								event.currentTarget.blur();
							}
						}}
					/>
				</div>
				<div class="flex w-full flex-col gap-1">
					<!-- The hidden copy grows the field with its text, as the name's does; its
						trailing space gives a last empty line its height. -->
					<div class="relative">
						<span
							class="invisible block min-h-7 px-2 py-1 text-sm leading-(--leading) wrap-break-word whitespace-pre-wrap"
							aria-hidden="true">{`${bio || BIO_PLACEHOLDER} `}</span
						>
						<textarea
							class="absolute inset-0 size-full resize-none overflow-hidden rounded-sm bg-transparent px-2 py-1 text-center text-sm leading-(--leading) text-fg-muted transition-colors placeholder:text-fg-subtle hover:bg-bg-hover focus:bg-transparent focus:text-fg focus:shadow-focus focus:outline-none"
							rows="1"
							placeholder={BIO_PLACEHOLDER}
							aria-label="Bio"
							aria-describedby={bioHint}
							enterkeyhint="done"
							readonly={savingBio}
							bind:value={bio}
							onchange={() => void commitBio()}
							onkeydown={(event) => {
								if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
									event.preventDefault();
									event.currentTarget.blur();
								}
								if (event.key === 'Escape') {
									bio = savedBio;
									event.currentTarget.blur();
								}
							}}></textarea>
					</div>
					<p class="text-center text-xs text-fg-subtle" id={bioHint}>Every agent reads this.</p>
				</div>
			</section>
		{/if}
	{/snippet}
</ResourceState>

<input class="hidden" type="file" accept="image/*" bind:this={picker} onchange={onpick} />

<ConfirmDialog
	bind:open={removing}
	title="Remove your photo?"
	description="This can’t be undone."
	confirmLabel="Remove"
	onconfirm={() => profile.setPhoto(null)}
/>
