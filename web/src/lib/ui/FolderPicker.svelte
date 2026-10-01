<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { get, post, message } from '$lib/api';
	import Modal from './Modal.svelte';
	import Button from './Button.svelte';
	import Icon from './Icon.svelte';

	// Choose a folder on a computer, as a tree (folders open as you go). Used when moving
	// files and when an app asks where to save. `suggest` adds a new folder to make,
	// marked "New", already chosen.
	let {
		computer,
		computerName = 'My computer',
		title,
		lede,
		action = 'Save here',
		start = '',
		exclude,
		suggest,
		onpick,
		onclose
	}: {
		computer: string;
		computerName?: string;
		title: string;
		lede?: string;
		action?: string;
		start?: string;
		/** A folder that can't be chosen, nor anything in it (the one being moved). */
		exclude?: string;
		suggest?: string;
		onpick: (path: string) => void;
		onclose: () => void;
	} = $props();

	type Node = { path: string; name: string; open: boolean; loaded: boolean; children: Node[]; fresh?: boolean };
	// Where the picker starts; later changes to the props don't move it.
	const initial = untrack(() => ({ name: computerName, start }));
	const root: Node = $state({ path: '', name: initial.name, open: true, loaded: false, children: [] });
	let chosen = $state(initial.start);
	let error = $state('');
	let making = $state(false);
	let newName = $state('');

	async function load(n: Node) {
		try {
			const r = await get(`/computers/${computer}/files?path=${encodeURIComponent(n.path)}`);
			const fresh = n.children.filter((c) => c.fresh);
			n.children = [
				...fresh,
				...r.entries
					.filter((e: { is_dir: boolean; name: string; path: string }) => e.is_dir && e.path !== exclude && !fresh.some((f) => f.name === e.name))
					.map((e: { path: string; name: string }) => ({ path: e.path, name: e.name, open: false, loaded: false, children: [] }))
			];
			n.loaded = true;
		} catch (e) {
			error = message(e);
		}
	}
	async function toggle(n: Node) {
		n.open = !n.open;
		if (n.open && !n.loaded) await load(n);
	}
	onMount(async () => {
		await load(root);
		// Open the way down to the starting folder, and add the suggested new one.
		let n = root;
		for (const part of start ? start.split('/') : []) {
			const next = n.children.find((c) => c.name === part);
			if (!next) break;
			next.open = true;
			if (!next.loaded) await load(next);
			n = next;
		}
		if (suggest && !n.children.some((c) => c.name === suggest)) {
			const path = n.path ? `${n.path}/${suggest}` : suggest;
			n.children = [{ path, name: suggest, open: false, loaded: true, children: [], fresh: true }, ...n.children];
			chosen = path;
		}
	});

	function find(n: Node, path: string): Node | null {
		if (n.path === path) return n;
		for (const c of n.children) {
			const f = find(c, path);
			if (f) return f;
		}
		return null;
	}
	function addFolder() {
		const name = newName.trim();
		if (!name || name.includes('/')) return;
		const parent = find(root, chosen) ?? root;
		const path = parent.path ? `${parent.path}/${name}` : name;
		parent.open = true;
		parent.children = [{ path, name, open: false, loaded: true, children: [], fresh: true }, ...parent.children];
		chosen = path;
		making = false;
		newName = '';
	}
	async function pick() {
		try {
			// Folders made here exist only once you choose them.
			if (find(root, chosen)?.fresh) await post(`/computers/${computer}/files/mkdir`, { path: chosen });
			onpick(chosen);
		} catch (e) {
			error = message(e);
		}
	}
</script>

{#snippet branch(n: Node, depth: number)}
	<div class="node" class:on={chosen === n.path} style="padding-left: {12 + depth * 18}px">
		<button class="tw" onclick={() => toggle(n)} aria-label={n.open ? `Close ${n.name}` : `Open ${n.name}`}><Icon name={n.open ? 'chevron-down' : 'chevron-right'} size={12} /></button>
		<button class="pickrow" onclick={() => (chosen = n.path)} aria-pressed={chosen === n.path}>
			<Icon name="folder" size={14} /><span class="nm">{n.name}</span>{#if n.fresh}<span class="new">New</span>{/if}
		</button>
	</div>
	{#if n.open}{#each n.children as c (c.path)}{@render branch(c, depth + 1)}{/each}{/if}
{/snippet}

<Modal {title} {lede} {onclose}>
	<div class="tree" role="tree" aria-label="Folders">{@render branch(root, 0)}</div>
	{#if error}<div class="error">{error}</div>{/if}
	<div class="where">
		<span class="mid">Saving to <span class="mono strong">/{chosen}</span></span>
		{#if making}
			<form class="nf" onsubmit={(e) => (e.preventDefault(), addFolder())}>
				<input bind:value={newName} placeholder="Folder name" aria-label="New folder name" />
				<Button size="sm" type="submit">Add</Button>
			</form>
		{:else}
			<Button variant="quiet" size="sm" icon="plus" onclick={() => (making = true)}>New folder</Button>
		{/if}
	</div>
	<div class="acts">
		<Button onclick={onclose}>Cancel</Button>
		<Button variant="primary" icon="check" onclick={pick}>{action}</Button>
	</div>
</Modal>

<style>
	.tree {
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--pane);
		padding: 6px;
		max-height: 300px;
		overflow: auto;
	}
	.node {
		display: flex;
		align-items: center;
		gap: 4px;
		height: 32px;
		border-radius: 6px;
		border: 1px solid transparent;
	}
	.node.on {
		background: var(--raised);
		border-color: var(--line-strong);
	}
	.tw {
		width: 18px;
		height: 18px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: none;
		border: 0;
		color: var(--low);
		cursor: pointer;
		padding: 0;
	}
	.pickrow {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 8px;
		background: none;
		border: 0;
		color: var(--ink);
		font: 14px Inter, system-ui, sans-serif;
		cursor: pointer;
		text-align: left;
		height: 100%;
		padding-right: 10px;
	}
	.nm {
		flex: 1;
	}
	.new {
		font-size: 11px;
		color: var(--mid);
	}
	.where {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		font-size: 13px;
	}
	.strong {
		color: var(--ink);
	}
	.nf {
		display: flex;
		gap: 6px;
	}
	.nf input {
		height: 30px;
		width: 160px;
	}
	.acts {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
