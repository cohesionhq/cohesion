// Reading a stored thread for display: each message's item as what the
// transcript renders, with tool calls paired to their results.

import type { IntelligenceThreadMessage, ThreadItem } from '$lib/protocol/types';

export type ParsedMessage =
	| { type: 'user'; id: number; text: string }
	| { type: 'assistant'; id: number; text: string }
	| { type: 'thinking'; id: number; text: string; redacted: boolean }
	| {
			type: 'toolRequest';
			id: number;
			call_identifier: string;
			tool_name: string;
			/** The `code` argument of the code tool, else null. */
			code: string | null;
			/** The arguments, pretty-printed when they are JSON. */
			arguments: string;
	  }
	| { type: 'toolResponse'; id: number; call_identifier: string; ok: boolean; text: string };

/** One row of the rendered transcript: a tool request carries its response. */
export type TranscriptEntry =
	| Exclude<ParsedMessage, { type: 'toolRequest' | 'toolResponse' }>
	| (Extract<ParsedMessage, { type: 'toolRequest' }> & {
			response: Extract<ParsedMessage, { type: 'toolResponse' }> | null;
	  })
	| Extract<ParsedMessage, { type: 'toolResponse' }>;

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

function parseJson(text: string): { ok: true; value: Json } | { ok: false } {
	try {
		return { ok: true, value: JSON.parse(text) as Json };
	} catch {
		return { ok: false };
	}
}

/** JSON text pretty-printed; anything else unchanged. */
export function prettyJson(text: string): string {
	const parsed = parseJson(text);
	if (!parsed.ok || typeof parsed.value === 'string') return text;
	return JSON.stringify(parsed.value, null, 2);
}

/** The `code` argument of a call's JSON arguments, if they have one. */
function codeArgument(args: string): string | null {
	const parsed = parseJson(args);
	if (!parsed.ok || typeof parsed.value !== 'object' || parsed.value === null) return null;
	if (Array.isArray(parsed.value)) return null;
	const code = parsed.value.code;
	return typeof code === 'string' ? code : null;
}

function parsed(id: number, item: ThreadItem): ParsedMessage {
	switch (item.kind) {
		case 'userMessage':
			return { type: 'user', id, text: item.text };
		case 'assistantMessage':
			return { type: 'assistant', id, text: item.text };
		case 'thought':
			return { type: 'thinking', id, text: item.text, redacted: false };
		case 'redactedThought':
			return { type: 'thinking', id, text: item.summary ?? '', redacted: true };
		case 'toolCallRequest':
			return {
				type: 'toolRequest',
				id,
				call_identifier: item.call_identifier,
				tool_name: item.tool_name,
				code: codeArgument(item.arguments),
				arguments: prettyJson(item.arguments)
			};
		case 'toolCallResponse':
			return {
				type: 'toolResponse',
				id,
				call_identifier: item.call_identifier,
				ok: item.succeeded,
				text: item.text
			};
	}
}

/** One stored message as what the transcript renders. */
export function parseMessage(message: IntelligenceThreadMessage): ParsedMessage {
	return parsed(message.id, message.item);
}

/** Attaches each tool response to the request with its `call_identifier`. */
export function pairToolCalls(messages: ParsedMessage[]): TranscriptEntry[] {
	const entries: TranscriptEntry[] = [];
	const open = new Map<string, Extract<TranscriptEntry, { type: 'toolRequest' }>>();
	for (const message of messages) {
		if (message.type === 'toolRequest') {
			const entry = { ...message, response: null };
			open.set(message.call_identifier, entry);
			entries.push(entry);
		} else if (message.type === 'toolResponse' && open.has(message.call_identifier)) {
			open.get(message.call_identifier)!.response = message;
			open.delete(message.call_identifier);
		} else {
			entries.push(message);
		}
	}
	return entries;
}

export function transcript(messages: IntelligenceThreadMessage[]): TranscriptEntry[] {
	return pairToolCalls(messages.map(parseMessage));
}
