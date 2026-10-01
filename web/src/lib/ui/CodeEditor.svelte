<script lang="ts">
	import { onMount } from 'svelte';
	import { EditorState, type Extension } from '@codemirror/state';
	import { EditorView, keymap, lineNumbers, highlightActiveLine } from '@codemirror/view';
	import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
	import { HighlightStyle, syntaxHighlighting, bracketMatching, indentOnInput } from '@codemirror/language';
	import { tags } from '@lezer/highlight';
	import { html } from '@codemirror/lang-html';
	import { css } from '@codemirror/lang-css';
	import { javascript } from '@codemirror/lang-javascript';
	import { python } from '@codemirror/lang-python';
	import { markdown } from '@codemirror/lang-markdown';

	let { value, path, onchange, onsave }: { value: string; path: string; onchange: (v: string) => void; onsave: () => void } = $props();
	let host: HTMLDivElement;
	let view: EditorView | undefined;

	// Syntax colors come from the scheme's tokens, like everything else.
	const highlight = HighlightStyle.define([
		{ tag: [tags.keyword, tags.modifier, tags.operatorKeyword], color: 'var(--chart-4)' },
		{ tag: [tags.string, tags.special(tags.string)], color: 'var(--chart-1)' },
		{ tag: [tags.number, tags.bool, tags.null], color: 'var(--chart-3)' },
		{ tag: [tags.comment], color: 'var(--low)', fontStyle: 'italic' },
		{ tag: [tags.tagName, tags.typeName, tags.className], color: 'var(--chart-2)' },
		{ tag: [tags.attributeName, tags.propertyName], color: 'var(--accent-ink)' },
		{ tag: [tags.function(tags.variableName), tags.definition(tags.variableName)], color: 'var(--ink)', fontWeight: '500' },
		{ tag: [tags.heading], color: 'var(--ink)', fontWeight: '600' }
	]);
	const theme = EditorView.theme({
		'&': { color: 'var(--ink)', backgroundColor: 'var(--pane)', height: '100%' },
		'.cm-content': { fontFamily: "'JetBrains Mono', monospace", fontSize: '12.5px', caretColor: 'var(--ink)' },
		'.cm-gutters': { backgroundColor: 'var(--pane)', color: 'var(--low)', border: 'none' },
		'.cm-activeLine': { backgroundColor: 'var(--surface)' },
		'.cm-activeLineGutter': { backgroundColor: 'var(--surface)' },
		'&.cm-focused .cm-selectionBackground, .cm-selectionBackground': { backgroundColor: 'var(--accent-soft)' },
		'.cm-cursor': { borderLeftColor: 'var(--ink)' },
		'&.cm-focused': { outline: 'none' }
	});

	function language(p: string): Extension {
		const ext = p.split('.').pop()?.toLowerCase();
		if (ext === 'html' || ext === 'htm' || ext === 'svelte') return html();
		if (ext === 'css') return css();
		if (ext === 'js' || ext === 'mjs' || ext === 'cjs' || ext === 'ts' || ext === 'json') return javascript({ typescript: ext === 'ts' });
		if (ext === 'py') return python();
		if (ext === 'md') return markdown();
		return [];
	}

	function build(doc: string) {
		view?.destroy();
		view = new EditorView({
			parent: host,
			state: EditorState.create({
				doc,
				extensions: [
					lineNumbers(),
					highlightActiveLine(),
					history(),
					bracketMatching(),
					indentOnInput(),
					keymap.of([{ key: 'Mod-s', run: () => (onsave(), true) }, indentWithTab, ...defaultKeymap, ...historyKeymap]),
					syntaxHighlighting(highlight),
					theme,
					language(path),
					EditorView.updateListener.of((u) => u.docChanged && onchange(u.state.doc.toString()))
				]
			})
		});
	}

	onMount(() => {
		build(value);
		return () => view?.destroy();
	});

	let shownPath = '';
	$effect(() => {
		// A different file was opened: start a fresh editor for it.
		if (view && path !== shownPath) {
			shownPath = path;
			build(value);
		}
	});
</script>

<div class="editor" bind:this={host} data-testid="editor"></div>

<style>
	.editor {
		height: 100%;
		min-height: 360px;
		border: 1px solid var(--line);
		border-radius: 10px;
		overflow: hidden;
	}
	.editor :global(.cm-editor) {
		height: 100%;
	}
</style>
