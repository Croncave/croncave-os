<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { get, post, api, message } from '$lib/api';
	import { refreshMe, session } from '$lib/session.svelte';
	import { keepAwake } from '$lib/presence';
	import { uploadFile } from '$lib/upload';
	import { bytes, clock } from '$lib/format';
	import AppWindow from '$lib/shell/AppWindow.svelte';
	import Button from '$lib/ui/Button.svelte';
	import FolderPicker from '$lib/ui/FolderPicker.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// Files: the computer's own folders, Recent, Trash and pinned folders; a preview beside
	// the list; everything else on the right-click menu.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	type Entry = any;
	const path = $derived(page.url.searchParams.get('path') ?? '');
	const view = $derived((page.url.searchParams.get('view') ?? 'files') as 'files' | 'recent' | 'trash');
	let entries = $state<Entry[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selected = $state<Entry | null>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let preview = $state<any>(null);
	let trash = $state<Entry[]>([]);
	let keepDays = $state(30);
	let usage = $state<{ files_bytes: number; trash_bytes: number } | null>(null);
	let uploads = $state<{ name: string; sent: number; size: number; error?: string }[]>([]);
	let dragging = $state(false);
	let q = $state('');
	let menu = $state<{ x: number; y: number; e: Entry } | null>(null);
	let newMenu = $state(false);
	let ask = $state<{ title: string; label: string; value: string; action: string; run: (v: string) => Promise<void> } | null>(null);
	let moving = $state<Entry | null>(null);
	let copying = $state<Entry | null>(null);
	let copyTo = $state('');
	let fileInput: HTMLInputElement | undefined = $state();

	$effect(() => keepAwake(() => session.computerId, 'files', 'files'));

	const c = $derived(session.me?.computers.find((x) => x.id === session.computerId));
	const others = $derived(session.me?.computers.filter((x) => x.id !== session.computerId) ?? []);
	const pins = $derived<string[]>((session.me?.user.prefs.pins ?? {})[session.computerId] ?? []);
	const base = $derived(`/computers/${session.computerId}/files`);

	async function load() {
		if (!session.computerId) return;
		loading = true;
		try {
			if (view === 'trash') {
				const t = await get(`${base}/trash`);
				trash = t.items;
				keepDays = t.keep_days;
			}
			else if (view === 'recent') entries = (await get(`${base}/recent?limit=50`)).entries;
			else entries = (await get(`${base}?path=${encodeURIComponent(path)}`)).entries;
			error = '';
			usage = await get(`${base}/usage`);
			const open = page.url.searchParams.get('open');
			if (open) {
				const e = entries.find((x) => x.path === open);
				if (e) select(e);
			}
		} catch (e) {
			error = message(e);
		} finally {
			loading = false;
		}
	}
	$effect(() => {
		void path;
		void view;
		void session.computerId;
		selected = null;
		preview = null;
		q = '';
		load();
	});

	const go = (p: string) => goto(`/files?path=${encodeURIComponent(p)}`);
	const crumbs = $derived(path ? path.split('/').map((name, i, all) => ({ name, path: all.slice(0, i + 1).join('/') })) : []);
	const shown = $derived(q.trim() ? entries.filter((e) => e.name.toLowerCase().includes(q.trim().toLowerCase())) : entries);
	const here = (name: string) => (path ? `${path}/${name}` : name);
	const parentOf = (p: string) => p.split('/').slice(0, -1).join('/');

	async function select(e: Entry) {
		if (e.is_dir) return go(e.path);
		selected = e;
		preview = null;
		try {
			preview = await get(`${base}/preview?path=${encodeURIComponent(e.path)}`);
		} catch (err) {
			preview = { kind: 'unsupported', reason: message(err) };
		}
	}

	async function upload(files: FileList | File[]) {
		for (const f of Array.from(files)) {
			const item: { name: string; sent: number; size: number; error?: string } = { name: f.name, sent: 0, size: f.size };
			uploads = [...uploads, item];
			try {
				await uploadFile(session.computerId, f, here(f.name), (sent) => {
					item.sent = sent;
					uploads = [...uploads];
				});
				toast(`Uploaded ${f.name}`);
			} catch (e) {
				item.error = message(e);
				uploads = [...uploads];
			}
		}
		uploads = uploads.filter((u) => u.error);
		load();
	}

	async function act(fn: () => Promise<unknown>, done?: string) {
		try {
			await fn();
			if (done) toast(done);
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	function newFolder() {
		newMenu = false;
		ask = { title: 'New folder', label: 'Folder name', value: '', action: 'Create', run: (v) => post(`${base}/mkdir`, { path: here(v) }) };
	}
	function newText() {
		newMenu = false;
		ask = {
			title: 'New text file',
			label: 'File name',
			value: 'notes.txt',
			action: 'Create',
			run: (v) => api(`${base}/write?path=${encodeURIComponent(here(v))}`, { method: 'PUT', raw: '' })
		};
	}
	function newScript() {
		newMenu = false;
		ask = {
			title: 'New script',
			label: 'File name (.py, .js or .sh)',
			value: 'script.py',
			action: 'Create and set it up',
			run: async (v) => {
				const starter = v.endsWith('.sh') ? '#!/usr/bin/env bash\necho "Hello from Croncave"\n' : v.endsWith('.js') ? 'console.log("Hello from Croncave");\n' : 'print("Hello from Croncave")\n';
				await api(`${base}/write?path=${encodeURIComponent(here(v))}`, { method: 'PUT', raw: starter });
				goto(`/scripts?add=1&path=${encodeURIComponent(here(v))}`);
			}
		};
	}
	function rename(e: Entry) {
		menu = null;
		ask = {
			title: `Rename ${e.is_dir ? 'folder' : 'file'}`,
			label: 'New name',
			value: e.name,
			action: 'Rename',
			run: (v) => post(`${base}/move`, { from: e.path, to: parentOf(e.path) ? `${parentOf(e.path)}/${v}` : v })
		};
	}
	async function submitAsk(ev: SubmitEvent) {
		ev.preventDefault();
		if (!ask || !ask.value.trim()) return;
		const a = ask;
		try {
			await a.run(a.value.trim());
			ask = null;
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	const trashIt = (e: Entry) => ((menu = null), (selected = null), act(() => post(`${base}/delete`, { path: e.path }), `Moved ${e.name} to Trash`));
	const restore = (t: Entry) => act(() => post(`${base}/restore`, { trash_id: t.id }), 'Restored');
	function emptyTrash() {
		if (confirm('Delete everything in Trash for good? This can’t be undone.')) act(() => post(`${base}/empty-trash`), 'Trash emptied');
	}
	async function togglePin(e: Entry) {
		menu = null;
		const all = { ...(session.me?.user.prefs.pins ?? {}) };
		const mine: string[] = all[session.computerId] ?? [];
		all[session.computerId] = mine.includes(e.path) ? mine.filter((p) => p !== e.path) : [...mine, e.path].slice(-20);
		await act(() => post('/me/prefs', { pins: all }).then(refreshMe), mine.includes(e.path) ? 'Unpinned' : `Pinned ${e.name}`);
	}
	async function moveTo(dest: string) {
		const e = moving!;
		moving = null;
		await act(() => post(`${base}/move`, { from: e.path, to: dest ? `${dest}/${e.name}` : e.name }), `Moved ${e.name}`);
	}
	async function copy() {
		const e = copying!;
		const to = others.find((o) => o.id === copyTo);
		copying = null;
		toast(`Copying ${e.name} to ${to?.name}…`);
		await act(() => post(`${base}/copy`, { path: e.path, to_computer: copyTo }), `Copied ${e.name} to ${to?.name}`);
	}
	const download = (e: Entry) => ((menu = null), (location.href = `/api${base}/download?path=${encodeURIComponent(e.path)}`));

	function contextMenu(ev: MouseEvent, e: Entry) {
		ev.preventDefault();
		menu = { x: Math.min(ev.clientX, window.innerWidth - 250), y: Math.min(ev.clientY, window.innerHeight - 300), e };
	}
	function keys(ev: KeyboardEvent) {
		const t = ev.target as HTMLElement;
		if (t.closest('input, textarea, [role=dialog]')) return;
		if (ev.key === 'Escape') ((menu = null), (newMenu = false));
		const e = menu?.e ?? selected;
		if (!e) return;
		if (ev.key === 'F2') (ev.preventDefault(), rename(e));
		if (ev.key === 'Delete' || (ev.key === 'Backspace' && (ev.metaKey || ev.ctrlKey))) (ev.preventDefault(), trashIt(e));
	}

	function kindOf(p: { kind: string; total_rows?: number; columns?: string[]; original_width?: number; original_height?: number }) {
		if (p.kind === 'table') return `Table · ${p.total_rows?.toLocaleString('en-US')} rows · ${p.columns?.length} columns`;
		if (p.kind === 'image') return `Image · ${p.original_width} × ${p.original_height}`;
		if (p.kind === 'text') return 'Text';
		return 'File';
	}
	const when = (ms: number) => {
		const s = clock(new Date(ms).toISOString());
		return s.includes(':') ? `Today ${s}` : s;
	};
	const from = (e: Entry) => e.source?.label?.replace(/^Made by "(.*)"$/, '$1') ?? '';
	const fromIcon = (e: Entry) => (e.source?.actor === 'run' ? 'code' : e.source?.actor === 'agent' ? 'sparkle' : e.source?.change === 'created' ? 'upload' : e.source ? 'pencil' : '');
	const size = (e: Entry) => (e.is_dir ? `${e.items ?? 0} ${e.items === 1 ? 'item' : 'items'}` : bytes(e.size));
	const gb = (n: number) => (n / 1e9 < 10 ? (n / 1e9).toFixed(1).replace(/\.0$/, '') : Math.round(n / 1e9).toString());
</script>

<svelte:window onkeydown={keys} onclick={(e) => { if (!(e.target as HTMLElement).closest('.ctx, .newwrap')) ((menu = null), (newMenu = false)); }} />

<AppWindow icon="folder" title="Files">
	<nav class="places" aria-label="Places">
		<a class="place" class:on={view === 'files' && !pins.includes(path)} href="/files"><Icon name="computer" size={15} />{c?.name ?? 'My computer'}</a>
		<a class="place" class:on={view === 'recent'} href="/files?view=recent"><Icon name="clock" size={15} />Recent</a>
		<a class="place" class:on={view === 'trash'} href="/files?view=trash"><Icon name="trash" size={15} /><span class="grow">Trash</span>{#if usage?.trash_bytes}<span class="mono low n">{bytes(usage.trash_bytes)}</span>{/if}</a>
		<span class="label pinned">Pinned</span>
		{#each pins as p (p)}
			<a class="place" class:on={view === 'files' && path === p} href="/files?path={encodeURIComponent(p)}" oncontextmenu={(ev) => contextMenu(ev, { path: p, name: p.split('/').at(-1), is_dir: true })}><Icon name="folder" size={15} />{p.split('/').at(-1)}</a>
		{:else}
			<span class="hint">Right-click a folder to pin it</span>
		{/each}
		<span class="grow"></span>
		{#if usage && c}
			<div class="storage">
				<div class="srow"><span class="label">Storage</span><span class="mono low">{gb(usage.files_bytes + usage.trash_bytes)} / {c.disk_gb} GB</span></div>
				<span class="sbar"><span style="width: {Math.min(100, ((usage.files_bytes + usage.trash_bytes) / (c.disk_gb * 1e9)) * 100)}%"></span></span>
				<a href="/settings" class="add">Add storage</a>
			</div>
		{/if}
	</nav>

	<section
		class="main"
		aria-label="Folder"
		ondragover={(e) => (e.preventDefault(), (dragging = view === 'files'))}
		ondragleave={(e) => e.currentTarget === e.target && (dragging = false)}
		ondrop={(e) => (e.preventDefault(), (dragging = false), view === 'files' && e.dataTransfer && upload(e.dataTransfer.files))}
	>
		{#if view === 'trash'}
			<div class="tool trash-head">
				<div class="th"><span class="tt">Trash</span><span class="mid small">Anything deleted by you, a script or an agent waits here for {keepDays} days, then it's gone for good.</span></div>
				<Button variant="danger" icon="trash" onclick={emptyTrash} disabled={!trash.length}>Empty Trash</Button>
			</div>
			<div class="table">
				<div class="hdr trashcols"><span>Name</span><span>Deleted by</span><span>Deleted</span><span>Days left</span><span></span></div>
				{#each trash as t (t.id)}
					<div class="r trashcols" data-testid="trash-row">
						<span class="name"><Icon name={t.is_dir ? 'folder' : 'file'} size={15} /><span title={t.original_path}>{t.original_path.split('/').at(-1)}</span><span class="low small">{parentOf(t.original_path) ? `in ${parentOf(t.original_path)}` : ''}</span></span>
						<span class="from">{#if t.run_id}<Icon name="code" size={12} /><a href="/runs/{t.run_id}">{t.deleted_by_label.replace(/^Deleted by /, '').replace(/^"(.*)"$/, '$1')}</a>{:else}<Icon name="upload" size={12} />{t.deleted_by_label.replace(/^Deleted by /, '').replace(/^you$/, 'You')}{/if}</span>
						<span class="mono when">{when(t.deleted_ms)}</span>
						<span class="mono when">{t.days_left}</span>
						<span class="right"><Button size="sm" icon="restore" onclick={() => restore(t)}>Restore</Button></span>
					</div>
				{:else}
					<div class="empty-row">Trash is empty.</div>
				{/each}
			</div>
		{:else}
			<div class="tool">
				<div class="crumbs">
					{#if view === 'recent'}<span class="cur">Recent</span>
					{:else}
						<a href="/files" class:cur={!path}>{c?.name ?? 'My computer'}</a>
						{#each crumbs as cr, i (cr.path)}<Icon name="chevron-right" size={13} />{#if i === crumbs.length - 1}<span class="cur">{cr.name}</span>{:else}<a href="/files?path={encodeURIComponent(cr.path)}">{cr.name}</a>{/if}{/each}
					{/if}
				</div>
				<div class="tools">
					<label class="search"><Icon name="search" size={14} /><input bind:value={q} placeholder={view === 'recent' ? 'Search recent files' : 'Search this folder'} aria-label="Search this folder" /></label>
					{#if view === 'files'}
						<div class="newwrap">
							<Button icon="plus" onclick={() => (newMenu = !newMenu)}>New</Button>
							{#if newMenu}
								<div class="newmenu menu" role="menu">
									<button role="menuitem" onclick={newFolder}><Icon name="folder" size={14} />Folder</button>
									<button role="menuitem" onclick={newText}><Icon name="file" size={14} />Text file</button>
									<button role="menuitem" onclick={newScript}><Icon name="code" size={14} />Script</button>
								</div>
							{/if}
						</div>
						<input type="file" multiple bind:this={fileInput} onchange={(e) => e.currentTarget.files && upload(e.currentTarget.files)} hidden data-testid="upload-input" />
						<Button variant="primary" icon="upload" onclick={() => fileInput?.click()}>Upload</Button>
					{/if}
				</div>
			</div>
			{#if error}<div class="err">{error}</div>{/if}
			{#if loading && !entries.length}
				<div class="center mid">Opening your files… your computer wakes if it's asleep.</div>
			{:else if !entries.length && view === 'files' && !path}
				<div class="center">
					<div class="emptybox">
						<Icon name="folder" size={28} />
						<h2>Your computer's files live here</h2>
						<p>Organise them however you like. Drop files or folders here, or create your own. Apps ask before they save anything here.</p>
						<div class="row2"><Button variant="primary" icon="upload" onclick={() => fileInput?.click()}>Upload files</Button><Button icon="folder" onclick={newFolder}>New folder</Button></div>
					</div>
				</div>
			{:else}
				<div class="table">
					<div class="hdr cols"><span>Name</span><span>From</span><span>Modified</span><span class="right">Size</span></div>
					<div class="rows">
						{#each shown as e (e.path)}
							<button
								class="r cols"
								class:sel={selected?.path === e.path || menu?.e.path === e.path}
								onclick={() => select(e)}
								oncontextmenu={(ev) => contextMenu(ev, e)}
								data-testid="file-row"
								title={view === 'recent' ? e.path : undefined}
							>
								<span class="name"><Icon name={e.is_dir ? 'folder' : 'file'} size={15} /><span>{e.name}</span>{#if view === 'recent' && parentOf(e.path)}<span class="low small">in {parentOf(e.path)}</span>{/if}</span>
								<span class="from" title={e.source?.label}>{#if e.source}<Icon name={fromIcon(e)} size={12} />{from(e)}{#if e.source.actor === 'run'}<span class="low">&nbsp;· run at {clock(e.source.at)}</span>{/if}{/if}</span>
								<span class="mono when">{when(e.modified_ms)}</span>
								<span class="mono when right">{size(e)}</span>
							</button>
						{:else}
							<div class="empty-row">{q ? 'Nothing here matches.' : view === 'recent' ? 'Nothing has changed yet.' : 'This folder is empty. Upload files or drop them here.'}</div>
						{/each}
					</div>
				</div>
				{#each uploads as u (u.name)}
					<div class="up"><Icon name="upload" size={13} /><span>{u.name}</span><span class="grow"></span>{#if u.error}<span class="error">{u.error}</span>{:else}<span class="mono low">{Math.round((u.sent / Math.max(1, u.size)) * 100)}%</span>{/if}</div>
				{/each}
				<div class="foot">{entries.length} item{entries.length === 1 ? '' : 's'}{#if view === 'files'} · drag files here to upload{/if}</div>
			{/if}
		{/if}
		{#if dragging}<div class="drop">Drop to upload into /{path}</div>{/if}
	</section>

	{#if selected}
		<aside class="preview" aria-label="Preview">
			<div class="ph">
				<div class="pt"><span class="mono pn">{selected.name}</span><span class="mid small">{preview ? kindOf(preview) : 'Making a preview on your computer…'}</span></div>
				<button class="x" onclick={() => (selected = null)} aria-label="Close preview"><Icon name="x" size={15} /></button>
			</div>
			{#if !preview}
				<div class="pbox low">…</div>
			{:else if preview.kind === 'table'}
				<div class="pbox tablewrap" data-testid="preview-table">
					<table><thead><tr>{#each preview.columns as col, i (i)}<th>{col}</th>{/each}</tr></thead>
						<tbody>{#each preview.rows.slice(0, 12) as r, i (i)}<tr>{#each r as cell, j (j)}<td>{cell}</td>{/each}</tr>{/each}</tbody></table>
				</div>
				<span class="low small">Showing {Math.min(12, preview.rows.length)} of {preview.total_rows} rows</span>
			{:else if preview.kind === 'image'}
				<div class="pbox img"><img src={preview.data_url} alt={selected.name} data-testid="preview-image" /></div>
				<span class="low small mono">{preview.original_width} × {preview.original_height}</span>
			{:else if preview.kind === 'text'}
				<pre class="pbox text" data-testid="preview-text">{preview.text}</pre>
				{#if preview.truncated}<span class="low small">Showing the start of the file.</span>{/if}
			{:else}
				<div class="pbox low">{preview.reason}</div>
			{/if}
			<div class="pacts">
				{#if preview?.kind === 'table'}<Button variant="primary" icon="data" block disabled title="The Data app is coming soon">Open in Data</Button>{/if}
				<Button icon="download-line" block={preview?.kind !== 'table'} onclick={() => download(selected)}>Download</Button>
			</div>
			<div class="details">
				{#if selected.source}
					<div class="kv"><span>{selected.source.actor === 'run' ? 'Made by' : 'From'}</span><span>{#if selected.source.run_id}<a href="/runs/{selected.source.run_id}">{from(selected)}</a>, run at {clock(selected.source.at)}{:else}{selected.source.label}{/if}</span></div>
				{/if}
				<div class="kv"><span>Location</span><span class="mono">/{selected.path}</span></div>
				<div class="kv"><span>Size</span><span class="mono">{bytes(selected.size)}</span></div>
				<div class="kv"><span>Modified</span><span class="mono">{when(selected.modified_ms)}</span></div>
			</div>
		</aside>
	{/if}
</AppWindow>

{#if menu}
	{@const e = menu.e}
	<div class="ctx menu" role="menu" style="left: {menu.x}px; top: {menu.y}px">
		<button role="menuitem" onclick={() => ((menu = null), select(e))}><Icon name="folder" size={14} />Open</button>
		{#if e.is_dir}<button role="menuitem" onclick={() => togglePin(e)}><Icon name="pin" size={14} />{pins.includes(e.path) ? 'Unpin from sidebar' : 'Pin to sidebar'}</button>{/if}
		<span class="sep"></span>
		<button role="menuitem" onclick={() => rename(e)}><Icon name="pencil" size={14} />Rename<span class="kbd mono">F2</span></button>
		<button role="menuitem" onclick={() => ((moving = e), (menu = null))}><Icon name="move" size={14} />Move to…</button>
		<button role="menuitem" disabled={!others.length} title={others.length ? undefined : 'You have one computer'} onclick={() => ((copying = e), (copyTo = others[0]?.id ?? ''), (menu = null))}><Icon name="copy" size={14} />Copy to another computer</button>
		<button role="menuitem" onclick={() => download(e)}><Icon name="download-line" size={14} />{e.is_dir ? 'Download as zip' : 'Download'}</button>
		<span class="sep"></span>
		<button role="menuitem" class="danger" onclick={() => trashIt(e)}><Icon name="trash" size={14} />Move to Trash<span class="kbd mono">Del</span></button>
	</div>
{/if}

{#if ask}
	<Modal title={ask.title} onclose={() => (ask = null)}>
		<form class="askform" onsubmit={submitAsk}>
			<label class="field"><span>{ask.label}</span><!-- svelte-ignore a11y_autofocus --><input bind:value={ask.value} aria-label={ask.label} autofocus /></label>
			<div class="acts"><Button onclick={() => (ask = null)}>Cancel</Button><Button variant="primary" type="submit">{ask.action}</Button></div>
		</form>
	</Modal>
{/if}

{#if moving}
	<FolderPicker computer={session.computerId} computerName={c?.name} title="Move {moving.name} to…" action="Move here" start={parentOf(moving.path)} exclude={moving.is_dir ? moving.path : undefined} onpick={moveTo} onclose={() => (moving = null)} />
{/if}

{#if copying}
	<Modal title="Copy {copying.name} to another computer" lede="It's copied to the same place there. Anything already at that spot moves to that computer's Trash." onclose={() => (copying = null)}>
		<label class="field"><span>Copy to</span>
			<select bind:value={copyTo} aria-label="Computer">{#each others as o (o.id)}<option value={o.id}>{o.name} · {o.status_word}</option>{/each}</select>
		</label>
		<div class="acts"><Button onclick={() => (copying = null)}>Cancel</Button><Button variant="primary" icon="copy" onclick={copy}>Copy</Button></div>
	</Modal>
{/if}

<style>
	.places {
		width: 245px;
		flex-shrink: 0;
		background: var(--pane);
		border-right: 1px solid var(--line);
		padding: 14px 10px 16px;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.place {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 36px;
		padding: 0 10px;
		border-radius: 6px;
		border: 1px solid transparent;
		color: var(--mid);
		font-size: 14px;
		font-weight: 500;
	}
	.place:hover {
		color: var(--ink);
		text-decoration: none;
	}
	.place.on {
		background: var(--surface);
		border-color: var(--line);
		color: var(--ink);
	}
	.place .n {
		font-size: 11px;
	}
	.grow {
		flex: 1;
	}
	.pinned {
		padding: 14px 10px 6px;
	}
	.hint {
		padding: 0 10px;
		font-size: 13px;
		color: var(--low);
	}
	.storage {
		padding: 0 10px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.srow {
		display: flex;
		justify-content: space-between;
	}
	.srow .mono {
		font-size: 11px;
	}
	.sbar {
		height: 4px;
		border-radius: 2px;
		background: var(--line);
	}
	.sbar span {
		display: block;
		height: 4px;
		border-radius: 2px;
		background: var(--ink);
		min-width: 2px;
	}
	.add {
		font-size: 13px;
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.main {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		position: relative;
	}
	.tool {
		height: 58px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 0 18px;
		border-bottom: 1px solid var(--line);
	}
	.crumbs {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 15px;
		color: var(--mid);
		min-width: 0;
		flex-shrink: 0;
		max-width: 60%;
		overflow: hidden;
		white-space: nowrap;
	}
	.crumbs a {
		color: var(--mid);
	}
	.crumbs .cur {
		color: var(--ink);
		font-weight: 600;
	}
	.tools {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		flex-shrink: 1;
	}
	.search {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 240px;
		min-width: 120px;
		flex-shrink: 1;
		height: 34px;
		padding: 0 10px;
		border: 1px solid var(--line);
		border-radius: 6px;
		background: var(--pane);
		color: var(--low);
	}
	.search input {
		border: 0;
		background: transparent;
		height: auto;
		padding: 0;
		font-size: 13px;
		box-shadow: none;
	}
	.newwrap {
		position: relative;
	}
	.menu {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 10px;
		box-shadow: 0 16px 40px var(--shadow);
		padding: 6px;
		display: flex;
		flex-direction: column;
		z-index: 40;
	}
	.newmenu {
		position: absolute;
		right: 0;
		top: 42px;
		width: 210px;
	}
	.ctx {
		position: fixed;
		width: 232px;
	}
	.menu button {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 32px;
		padding: 0 10px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--ink);
		font: 14px Inter, system-ui, sans-serif;
		text-align: left;
		cursor: pointer;
	}
	.menu button:hover:not(:disabled) {
		background: var(--raised);
	}
	.menu button:disabled {
		color: var(--low);
		cursor: default;
	}
	.menu .danger {
		color: var(--failed);
	}
	.kbd {
		margin-left: auto;
		font-size: 11px;
		color: var(--low);
	}
	.sep {
		height: 1px;
		background: var(--line);
		margin: 4px 2px;
	}
	.table {
		display: flex;
		flex-direction: column;
		min-height: 0;
		flex: 1;
	}
	.rows {
		overflow: auto;
		flex: 1;
	}
	.cols {
		display: grid;
		grid-template-columns: minmax(0, 1.6fr) minmax(0, 1.4fr) 110px 90px;
		gap: 12px;
		align-items: center;
	}
	.trashcols {
		display: grid;
		grid-template-columns: minmax(0, 1.6fr) minmax(0, 1.4fr) 140px 90px 110px;
		gap: 12px;
		align-items: center;
	}
	.hdr {
		padding: 10px 18px;
		border-bottom: 1px solid var(--line);
		font: 500 11px/16px 'JetBrains Mono', ui-monospace, monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--low);
	}
	.r {
		width: 100%;
		min-height: 38px;
		padding: 0 18px;
		border: 0;
		border-bottom: 1px solid var(--line);
		background: transparent;
		color: var(--ink);
		font: 15px Inter, system-ui, sans-serif;
		text-align: left;
		cursor: pointer;
	}
	.trashcols.r {
		min-height: 50px;
		cursor: default;
	}
	.r:hover {
		background: var(--pane);
	}
	.r.sel {
		background: var(--raised);
	}
	.name {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
	}
	.name > span:first-of-type {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.from {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--mid);
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.when {
		font-size: 12px;
		color: var(--mid);
	}
	.right {
		text-align: right;
		justify-self: end;
	}
	.small {
		font-size: 12px;
	}
	.empty-row {
		padding: 28px 18px;
		color: var(--mid);
	}
	.foot {
		flex-shrink: 0;
		padding: 10px 18px;
		border-top: 1px solid var(--line);
		font-size: 13px;
		color: var(--mid);
	}
	.up {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 18px;
		border-top: 1px solid var(--line);
		font-size: 13px;
	}
	.err {
		margin: 12px 18px 0;
		color: var(--failed);
	}
	.center {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 20px;
	}
	.emptybox {
		width: min(625px, 100%);
		padding: 80px 40px;
		border: 1.5px dashed var(--line-strong);
		border-radius: 12px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		text-align: center;
		color: var(--mid);
	}
	.emptybox h2 {
		font-size: 20px;
		color: var(--ink);
	}
	.emptybox p {
		max-width: 380px;
		font-size: 14px;
		line-height: 21px;
	}
	.row2 {
		display: flex;
		gap: 8px;
		margin-top: 12px;
	}
	.drop {
		position: absolute;
		inset: 12px;
		border: 2px dashed var(--accent);
		border-radius: 12px;
		background: var(--overlay);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		pointer-events: none;
	}
	.trash-head {
		height: auto;
		padding: 12px 18px;
	}
	.th {
		display: flex;
		flex-direction: column;
	}
	.tt {
		font-size: 15px;
		font-weight: 600;
	}
	.preview {
		width: 396px;
		flex-shrink: 0;
		border-left: 1px solid var(--line);
		padding: 16px 18px;
		display: flex;
		flex-direction: column;
		gap: 12px;
		overflow: auto;
	}
	.ph {
		display: flex;
		justify-content: space-between;
		gap: 8px;
	}
	.pt {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.pn {
		font-size: 14px;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.x {
		width: 28px;
		height: 28px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--mid);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
	}
	.x:hover {
		background: var(--raised);
		color: var(--ink);
	}
	.pbox {
		border: 1px solid var(--line);
		border-radius: 8px;
		background: var(--pane);
		max-height: 340px;
		overflow: auto;
	}
	.tablewrap table {
		width: 100%;
		border-collapse: collapse;
		font-size: 13px;
	}
	.tablewrap th {
		font: 400 11px 'JetBrains Mono', ui-monospace, monospace;
		color: var(--low);
		text-align: left;
		padding: 6px 10px;
		border-bottom: 1px solid var(--line);
		white-space: nowrap;
	}
	.tablewrap td {
		padding: 5px 10px;
		border-bottom: 1px solid var(--line);
		white-space: nowrap;
	}
	.img {
		display: flex;
		justify-content: center;
		padding: 10px;
	}
	.img img {
		max-width: 100%;
		image-rendering: pixelated;
	}
	.text {
		margin: 0;
		padding: 10px 12px;
		font-size: 12px;
		white-space: pre-wrap;
	}
	.pacts {
		display: flex;
		gap: 8px;
	}
	.details {
		display: flex;
		flex-direction: column;
	}
	.kv {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 8px 0;
		border-bottom: 1px solid var(--line);
		font-size: 13px;
		color: var(--mid);
	}
	.kv > span:last-child {
		color: var(--ink);
		text-align: right;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.kv .mono {
		font-size: 12px;
	}
	.askform {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 13px;
		font-weight: 500;
	}
	.acts {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
	@media (max-width: 1100px) {
		.preview {
			position: fixed;
			right: 12px;
			top: 64px;
			bottom: 12px;
			background: var(--surface);
			border: 1px solid var(--line);
			border-radius: 12px;
			z-index: 30;
			box-shadow: 0 16px 40px var(--shadow);
		}
	}
	@media (max-width: 760px) {
		.places {
			display: none;
		}
		.cols {
			grid-template-columns: minmax(0, 1fr) 90px;
		}
		.cols > :nth-child(2),
		.cols > :nth-child(3) {
			display: none;
		}
		.search {
			width: 140px;
		}
	}
</style>
