// Settings → Developer: the script runner, a raw API console and the
// `cohesion.d.ts` the LLM code tool sees. Drafts and results live here so
// they survive switching sections.

import { send as defaultSend, sendRaw } from '$lib/protocol/api';
import { toProtocolError, type ProtocolError } from '$lib/protocol/errors';
import type { RawRequest, ReplyFrame, Send } from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

export const DEFAULT_SCRIPT = 'cohesion.workspaces().map(w => w.title)';

/** JSON text indented for reading; anything that isn't JSON comes back as is. */
export function prettyJson(text: string): string {
	try {
		return JSON.stringify(JSON.parse(text), null, 2);
	} catch {
		return text;
	}
}

/** Parses the console's `args` text: blank means no args. */
export function parseArgs(
	text: string
): { ok: true; value: unknown } | { ok: false; message: string } {
	if (text.trim() === '') return { ok: true, value: undefined };
	try {
		return { ok: true, value: JSON.parse(text) };
	} catch (error) {
		return { ok: false, message: error instanceof Error ? error.message : String(error) };
	}
}

export type FrameSender = (request: RawRequest) => Promise<ReplyFrame>;

export class DeveloperStore {
	readonly documentation: Resource<string>;

	script = $state(DEFAULT_SCRIPT);
	running = $state(false);
	/** The last run's result, pretty-printed. */
	result = $state<string | null>(null);
	runError = $state<ProtocolError | null>(null);

	consoleType = $state('listWorkspaces');
	consoleArgs = $state('');
	consoleBusy = $state(false);
	consoleFrame = $state<string | null>(null);
	consoleError = $state<string | null>(null);

	#send: Send;
	#frame: FrameSender;

	constructor(send: Send, frame: FrameSender = sendRaw) {
		this.#send = send;
		this.#frame = frame;
		this.documentation = new Resource({
			fetch: () => send('documentation').then((reply) => reply.documentation),
			initial: ''
		});
	}

	async run(): Promise<void> {
		if (this.running) return;
		this.running = true;
		this.runError = null;
		try {
			this.result = prettyJson(await this.#send('runScript', { script: this.script }));
		} catch (error) {
			this.result = null;
			this.runError = toProtocolError(error);
		} finally {
			this.running = false;
		}
	}

	/** Sends the console's request and keeps the whole reply frame, errors included. */
	async sendConsole(): Promise<void> {
		const args = parseArgs(this.consoleArgs);
		if (!args.ok) {
			this.consoleError = `args isn’t JSON: ${args.message}`;
			return;
		}
		this.consoleBusy = true;
		this.consoleError = null;
		try {
			const type = this.consoleType.trim();
			const frame = await this.#frame(
				args.value === undefined ? { type } : { type, args: args.value }
			);
			this.consoleFrame = JSON.stringify(frame, null, 2);
		} catch (error) {
			this.consoleFrame = null;
			this.consoleError = toProtocolError(error).message;
		} finally {
			this.consoleBusy = false;
		}
	}
}

export function createDeveloperStore(
	send: Send = defaultSend,
	frame: FrameSender = sendRaw
): DeveloperStore {
	return new DeveloperStore(send, frame);
}

export const developer = createDeveloperStore();
