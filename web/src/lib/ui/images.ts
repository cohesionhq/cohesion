// Images attached to messages, to an agent or in an actor session, and photos
// of contacts and the user: read from a file, scaled down to what is looked at, and shown back
// from base64.

import type { ImageUpload } from '$lib/protocol/types';
import { toasts } from '$lib/ui/toasts.svelte';

/** The most images one message carries, as the daemon allows. */
export const MAX_IMAGES = 10;

/** The image types every model reads; anything else is re-encoded. */
const ACCEPTED_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

/** The longest side the models look at; a larger image is scaled down to it. */
const MAX_SIDE = 1568;

/** The largest file sent as it is: 5 MB once base64-encoded, the daemon's cap. */
const MAX_BYTES = (5 * 1024 * 1024 * 3) / 4;

/** The longest side of a photo of someone as sent: twice the largest disc
 * it shows on, for sharp screens, and small enough to keep listings light. */
export const PHOTO_SIDE = 512;

/** The images among `list`, such as a paste's or a drop's files. */
export function imageFiles(list: FileList | null | undefined): File[] {
	return [...(list ?? [])].filter((file) => file.type.startsWith('image/'));
}

/**
 * `images` with those among `files` added, up to {@link MAX_IMAGES} in all;
 * a toast says why when some don't fit or can't be read.
 */
export async function attachImages(images: ImageUpload[], files: File[]): Promise<ImageUpload[]> {
	const room = MAX_IMAGES - images.length;
	if (files.length > room) toasts.show(`A message can have up to ${MAX_IMAGES} images.`, 'error');
	const added: ImageUpload[] = [];
	for (const file of files.slice(0, Math.max(0, room))) {
		try {
			added.push(await prepareImage(file));
		} catch (error) {
			toasts.error(error);
		}
	}
	return [...images, ...added];
}

/**
 * `file` ready to attach: as it is when the models take it whole and it
 * fits `maxSide`, otherwise scaled to fit and re-encoded as JPEG on white.
 * Throws when the browser can't read it as an image.
 */
export async function prepareImage(file: Blob, maxSide = MAX_SIDE): Promise<ImageUpload> {
	let bitmap: ImageBitmap;
	try {
		bitmap = await createImageBitmap(file);
	} catch {
		throw new Error('That file isn’t an image that can be attached.');
	}
	const scale = Math.min(1, maxSide / Math.max(bitmap.width, bitmap.height));
	if (scale === 1 && ACCEPTED_TYPES.includes(file.type) && file.size <= MAX_BYTES) {
		bitmap.close();
		return { media_type: file.type, data: await base64(file) };
	}
	const canvas = document.createElement('canvas');
	canvas.width = Math.max(1, Math.round(bitmap.width * scale));
	canvas.height = Math.max(1, Math.round(bitmap.height * scale));
	const context = canvas.getContext('2d')!;
	context.fillStyle = '#fff';
	context.fillRect(0, 0, canvas.width, canvas.height);
	context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
	bitmap.close();
	const jpeg = await new Promise<Blob>((resolve, reject) =>
		canvas.toBlob(
			(blob) => (blob ? resolve(blob) : reject(new Error('Couldn’t read the image.'))),
			'image/jpeg',
			0.85
		)
	);
	return { media_type: 'image/jpeg', data: await base64(jpeg) };
}

/** The image as a `data:` URL, for an `<img>`. */
export function imageUrl(image: ImageUpload): string {
	return `data:${image.media_type};base64,${image.data}`;
}

function base64(blob: Blob): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => {
			const url = reader.result as string;
			resolve(url.slice(url.indexOf(',') + 1));
		};
		reader.onerror = () => reject(new Error('Couldn’t read the image.'));
		reader.readAsDataURL(blob);
	});
}
