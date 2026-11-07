import { describe, expect, it } from 'vitest';
import type { IntelligenceThreadMessage, ThreadItem } from '$lib/protocol/types';
import { parseMessage, transcript } from './transcript';

function message(item: ThreadItem, id = 1): IntelligenceThreadMessage {
	return { id, thread_id: 1, created_at: 0, item };
}

describe('parseMessage', () => {
	it('reads user and assistant text', () => {
		expect(parseMessage(message({ kind: 'userMessage', text: 'Hi' }))).toMatchObject({
			type: 'user',
			text: 'Hi'
		});
		expect(parseMessage(message({ kind: 'assistantMessage', text: '**Yes**' }))).toMatchObject({
			type: 'assistant',
			text: '**Yes**'
		});
	});

	it('reads standard and redacted thinking', () => {
		expect(
			parseMessage(message({ kind: 'thought', text: 'Hmm', signature: null, in_content: false }))
		).toMatchObject({ type: 'thinking', text: 'Hmm', redacted: false });
		expect(
			parseMessage(message({ kind: 'redactedThought', trace: 'xyz', summary: null }))
		).toMatchObject({ type: 'thinking', text: '', redacted: true });
	});

	it('reads tool calls and pairs them with their responses', () => {
		const request = message({
			kind: 'toolCallRequest',
			call_identifier: 'c1',
			tool_name: 'code-tool',
			arguments: JSON.stringify({ code: 'cohesion.workspaces()' })
		});
		const response = message(
			{ kind: 'toolCallResponse', call_identifier: 'c1', succeeded: false, text: 'boom' },
			2
		);
		expect(parseMessage(request)).toMatchObject({
			type: 'toolRequest',
			tool_name: 'code-tool',
			code: 'cohesion.workspaces()'
		});
		const entries = transcript([request, response]);
		expect(entries).toHaveLength(1);
		expect(entries[0]).toMatchObject({
			type: 'toolRequest',
			response: { type: 'toolResponse', ok: false, text: 'boom' }
		});
	});
});
