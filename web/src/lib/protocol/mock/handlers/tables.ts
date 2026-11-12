// Tables: a folder's properties, their options and formulas, and its rows and values.

import { ProtocolError } from '../../errors';
import type { IntelligenceAgentColor, PropertyValue } from '../../types';
import {
	checkedValue,
	computeFormulas,
	folderProperties,
	listTableRows,
	optionOfProperty,
	parseValue,
	storeValue,
	tableRow,
	typedProperty,
	valueText
} from '../tableRules';
import type { Handlers, MockCore } from '../core';

export function tablesHandlers(core: MockCore) {
	const {
		state,
		nextId,
		tableOf,
		propertyOf,
		rowOf,
		invalidate,
		tablesChanged,
		requirePropertyName,
		requireFormula,
		requireNotFormulaChoice,
		replaceProperty,
		choiceOf,
		requireOptionName,
		addOptions,
		optionIn,
		optionsChanged
	} = core;
	return {
		listDocumentFolderProperties: ({ workspace_id, folder_id }) => {
			tableOf(workspace_id, folder_id);
			return folderProperties(state, folder_id).map((p) => structuredClone(p));
		},
		createDocumentFolderProperty: ({
			workspace_id,
			folder_id,
			name,
			data_type,
			formula = null,
			options
		}) => {
			requirePropertyName(name);
			requireFormula(formula);
			requireNotFormulaChoice(data_type, formula !== null);
			if (data_type !== 'choice' && options.length > 0)
				throw new ProtocolError(
					'validationFailed',
					'Only a choice property has options.',
					'options'
				);
			const folder = tableOf(workspace_id, folder_id);
			const created = typedProperty(
				{
					id: nextId(state.folderProperties),
					folder_id,
					name,
					sort_order:
						folderProperties(state, folder_id).reduce((max, p) => Math.max(max, p.sort_order), 0) +
						1,
					formula: formula === null ? null : { ...formula, error: null }
				},
				data_type
			);
			if (created.data_type === 'choice') addOptions(created, options, 'options');
			state.folderProperties.push(created);
			computeFormulas(state, folder_id);
			tablesChanged(folder_id);
			// A folder showing as a list shows as a table from its new property on.
			if (folder.representation !== 'table') {
				folder.representation = 'table';
				invalidate({ kind: 'documentFolders', id: workspace_id });
			}
			return structuredClone(created);
		},
		renameDocumentFolderProperty: ({ workspace_id, folder_id, property_id, name }) => {
			requirePropertyName(name);
			const p = propertyOf(workspace_id, folder_id, property_id);
			p.name = name;
			computeFormulas(state, folder_id);
			tablesChanged(folder_id);
			return structuredClone(p);
		},
		setDocumentFolderPropertyType: ({ workspace_id, folder_id, property_id, data_type }) => {
			const p = propertyOf(workspace_id, folder_id, property_id);
			requireNotFormulaChoice(data_type, p.formula !== null);
			if (p.data_type === data_type) return structuredClone(p);
			// Each value as text, a choice's its option's name; the options go with the old type.
			const texts = state.documentValues
				.filter((v) => v.property_id === property_id)
				.map((v) => ({ document_id: v.document_id, text: valueText(p, v.value) }));
			const next = replaceProperty(p, typedProperty(p, data_type));
			if (next.data_type === 'choice') {
				// An option for each different text, ignoring case, alphabetically, with no color.
				const names: string[] = [];
				for (const { text } of texts)
					if (text !== '' && !names.some((n) => n.toLowerCase() === text.toLowerCase()))
						names.push(text);
				names.sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()) || (a < b ? -1 : 1));
				addOptions(
					next,
					names.map((name) => ({ name, color: null })),
					'name'
				);
				for (const { document_id, text } of texts) {
					const option = next.options.find((o) => o.name.toLowerCase() === text.toLowerCase());
					storeValue(
						state,
						document_id,
						property_id,
						option ? { type: 'choice', value: option.id } : null
					);
				}
			} else {
				for (const { document_id, text } of texts)
					storeValue(state, document_id, property_id, parseValue(data_type, text));
			}
			computeFormulas(state, folder_id);
			tablesChanged(folder_id);
			return structuredClone(next);
		},
		setDocumentFolderPropertyFormula: ({
			workspace_id,
			folder_id,
			property_id,
			formula = null
		}) => {
			requireFormula(formula);
			const p = propertyOf(workspace_id, folder_id, property_id);
			requireNotFormulaChoice(p.data_type, formula !== null);
			// The same source with another mode keeps its outputs and error, and computes nothing.
			if (formula !== null && p.formula?.source === formula.source) {
				p.formula = { ...p.formula, update_mode: formula.update_mode };
				tablesChanged(folder_id);
				return structuredClone(p);
			}
			// Becoming a formula drops the values set by hand; no longer one keeps its last outputs.
			if (formula !== null && p.formula === null)
				state.documentValues = state.documentValues.filter((v) => v.property_id !== property_id);
			p.formula = formula === null ? null : { ...formula, error: null };
			computeFormulas(state, folder_id);
			tablesChanged(folder_id);
			return structuredClone(p);
		},
		moveDocumentFolderProperty: ({ workspace_id, folder_id, property_id, before_id = null }) => {
			const moving = propertyOf(workspace_id, folder_id, property_id);
			const others = folderProperties(state, folder_id).filter((p) => p.id !== property_id);
			const at = before_id === null ? others.length : others.findIndex((p) => p.id === before_id);
			if (at < 0)
				throw new ProtocolError(
					'validationFailed',
					'The property to go before isn’t in that folder.'
				);
			others.splice(at, 0, moving);
			others.forEach((p, i) => (p.sort_order = i + 1));
			tablesChanged(folder_id);
			return others.map((p) => structuredClone(p));
		},
		deleteDocumentFolderProperty: ({ workspace_id, folder_id, property_id }) => {
			const p = propertyOf(workspace_id, folder_id, property_id);
			state.folderProperties = state.folderProperties.filter((x) => x.id !== property_id);
			state.documentValues = state.documentValues.filter((v) => v.property_id !== property_id);
			computeFormulas(state, folder_id);
			tablesChanged(folder_id);
			return structuredClone(p);
		},
		createPropertyOption: ({ workspace_id, folder_id, property_id, name, color = null }) => {
			const p = choiceOf(workspace_id, folder_id, property_id);
			addOptions(p, [{ name, color }], 'name');
			return optionsChanged(folder_id, p);
		},
		renamePropertyOption: ({ workspace_id, folder_id, property_id, option_id, name }) => {
			const p = choiceOf(workspace_id, folder_id, property_id);
			const option = optionIn(p, option_id);
			requireOptionName(p.options, name, 'name', option_id);
			option.name = name;
			return optionsChanged(folder_id, p);
		},
		setPropertyOptionColor: ({ workspace_id, folder_id, property_id, option_id, color = null }) => {
			const p = choiceOf(workspace_id, folder_id, property_id);
			optionIn(p, option_id).color = color as IntelligenceAgentColor | null;
			return optionsChanged(folder_id, p);
		},
		movePropertyOption: ({ workspace_id, folder_id, property_id, option_id, before_id = null }) => {
			const p = choiceOf(workspace_id, folder_id, property_id);
			const moving = optionIn(p, option_id);
			const others = p.options.filter((o) => o.id !== option_id);
			const at = before_id === null ? others.length : others.findIndex((o) => o.id === before_id);
			if (at < 0)
				throw new ProtocolError(
					'validationFailed',
					'The option to go before isn’t in that column.'
				);
			others.splice(at, 0, moving);
			others.forEach((o, i) => (o.sort_order = i));
			p.options = others;
			return optionsChanged(folder_id, p);
		},
		deletePropertyOption: ({ workspace_id, folder_id, property_id, option_id }) => {
			const p = choiceOf(workspace_id, folder_id, property_id);
			optionIn(p, option_id);
			p.options = p.options.filter((o) => o.id !== option_id);
			// The values holding it go with it.
			state.documentValues = state.documentValues.filter(
				(v) =>
					!(
						v.property_id === property_id &&
						v.value.type === 'choice' &&
						v.value.value === option_id
					)
			);
			return optionsChanged(folder_id, p);
		},
		listTableRows: ({ workspace_id, folder_id, filters, sort }) => {
			tableOf(workspace_id, folder_id);
			return listTableRows(state, folder_id, filters, sort).map((d) => tableRow(state, d));
		},
		getTableRow: ({ workspace_id, folder_id, document_id }) =>
			tableRow(state, rowOf(workspace_id, folder_id, document_id)),
		setDocumentValue: ({ workspace_id, folder_id, document_id, property_id, value = null }) => {
			const p = propertyOf(workspace_id, folder_id, property_id);
			if (p.formula !== null)
				throw new ProtocolError(
					'validationFailed',
					'A formula’s values can’t be set. Change its formula instead.',
					'property_id'
				);
			const d = rowOf(workspace_id, folder_id, document_id);
			if (value !== null && value.type !== p.data_type)
				throw new ProtocolError(
					'validationFailed',
					'The value must be of its property’s type.',
					'value'
				);
			if (value?.type === 'choice' && optionOfProperty(p, value.value) === null)
				throw new ProtocolError(
					'validationFailed',
					'The option isn’t one of this property’s.',
					'value'
				);
			let stored: PropertyValue | null;
			try {
				stored = value && checkedValue(value);
			} catch (e) {
				throw new ProtocolError('validationFailed', (e as Error).message, 'value');
			}
			storeValue(state, document_id, property_id, stored);
			computeFormulas(state, folder_id);
			tablesChanged(folder_id);
			return tableRow(state, d);
		}
	} satisfies Partial<Handlers>;
}
