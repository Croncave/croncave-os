<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { get, post, message } from '$lib/api';
	import { session } from '$lib/session.svelte';
	import { keepAwake } from '$lib/presence';
	import { uploadFile } from '$lib/upload';
	import { ago, bytes } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	type Entry = any;
	const path = $derived(page.url.searchParams.get('path') ?? '');
	let entries = $state<Entry[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selected = $state<Entry | null>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let preview = $state<any>(null);
	let trashView = $state(false);
	let trash = $state<Entry[]>([]);
	let usage = $state<{ files_bytes: number; trash_bytes: number } | null>(null);
	let uploads = $state<{ name: string; sent: number; size: number; error?: string }[]>([]);
	let dragging = $state(false);
	let rename = $state<Entry | null>(null);
	let newName = $state('');
	let fileInput: HTMLInputElement | undefined = $state();

	$effect(() => keepAwake(() => session.computerId, 'files', 'files'));

	async function load() {
		const c = session.computerId;
		if (!c) return;
		loading = true;
		try {
			const r = await get(`/computers/${c}/files?path=${encodeURIComponent(path)}`);
			entries = r.entries;
			error = '';
			usage = await get(`/computers/${c}/files/usage`);
			if (trashView) trash = (await get(`/computers/${c}/files/trash`)).items;
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
		void session.computerId;
		void trashView;
		selected = null;
		preview = null;
		load();
	});

	const go = (p: string) => goto(`/files?path=${encodeURIComponent(p)}`);
	const crumbs = $derived(path ? path.split('/').map((name, i, all) => ({ name, path: all.slice(0, i + 1).join('/') })) : []);

	async function select(e: Entry) {
		if (e.is_dir) return go(e.path);
		selected = e;
		preview = null;
		try {
			preview = await get(`/computers/${session.computerId}/files/preview?path=${encodeURIComponent(e.path)}`);
		} catch (err) {
			preview = { kind: 'unsupported', reason: message(err) };
		}
	}

	async function upload(files: FileList | File[]) {
		for (const f of Array.from(files)) {
			const item: { name: string; sent: number; size: number; error?: string } = { name: f.name, sent: 0, size: f.size };
			uploads = [...uploads, item];
			const target = path ? `${path}/${f.name}` : f.name;
			try {
				await uploadFile(session.computerId, f, target, (sent) => {
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

	async function mkdir() {
		const name = prompt('Folder name');
		if (!name) return;
		try {
			await post(`/computers/${session.computerId}/files/mkdir`, { path: path ? `${path}/${name}` : name });
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function remove(e: Entry) {
		try {
			await post(`/computers/${session.computerId}/files/delete`, { path: e.path });
			toast(`Moved ${e.name} to Trash`);
			selected = null;
			load();
		} catch (err) {
			toast(message(err), true);
		}
	}
	async function doRename() {
		if (!rename) return;
		const to = rename.path.split('/').slice(0, -1).concat(newName).join('/');
		try {
			await post(`/computers/${session.computerId}/files/move`, { from: rename.path, to });
			rename = null;
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function restore(t: Entry) {
		try {
			const r = await post(`/computers/${session.computerId}/files/restore`, { trash_id: t.id });
			toast(`Restored ${r.path}`);
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function emptyTrash() {
		if (!confirm('Delete everything in Trash for good?')) return;
		await post(`/computers/${session.computerId}/files/empty-trash`);
		load();
	}
	const c = $derived(session.me?.computers.find((x) => x.id === session.computerId));
</script>

<div
	class="page files"
	role="region"
	aria-label="Files"
	ondragover={(e) => (e.preventDefault(), (dragging = true))}
	ondragleave={() => (dragging = false)}
	ondrop={(e) => (e.preventDefault(), (dragging = false), e.dataTransfer && upload(e.dataTransfer.files))}
>
	<div class="page-head">
		<div class="stack" style="gap: 4px">
			<h1>Files</h1>
			<div class="crumbs row">
				<button class="crumb" onclick={() => ((trashView = false), go(''))}>{c?.name ?? 'Computer'}</button>
				{#each crumbs as cr (cr.path)}<span class="low">/</span><button class="crumb" onclick={() => go(cr.path)}>{cr.name}</button>{/each}
				{#if trashView}<span class="low">/</span><span>Trash</span>{/if}
			</div>
		</div>
		<div class="row">
			<input type="file" multiple bind:this={fileInput} onchange={(e) => e.currentTarget.files && upload(e.currentTarget.files)} hidden data-testid="upload-input" />
			<Button variant="primary" onclick={() => fileInput?.click()}>Upload</Button>
			<Button onclick={mkdir}>New folder</Button>
			<Button onclick={() => (trashView = !trashView)}>{trashView ? 'Back to files' : 'Trash'}</Button>
		</div>
	</div>
	{#if usage && c}
		{@const total = c.disk_gb * 1e9}
		<div class="usage">
			<div class="bar"><span class="f" style="width: {Math.min(100, (usage.files_bytes / total) * 100)}%"></span><span class="t" style="width: {Math.min(100, (usage.trash_bytes / total) * 100)}%"></span></div>
			<span class="low mono">{bytes(usage.files_bytes)} files · {bytes(usage.trash_bytes)} in Trash · {c.disk_gb} GB</span>
		</div>
	{/if}
	{#each uploads as u (u.name)}
		<div class="pane row"><span>{u.name}</span><span class="spacer"></span>{#if u.error}<span class="error">{u.error}</span>{:else}<span class="mono low">{Math.round((u.sent / Math.max(1, u.size)) * 100)}%</span>{/if}</div>
	{/each}
	{#if dragging}<div class="drop">Drop to upload into /{path}</div>{/if}
	{#if error}<div class="banner bad">{error}</div>{/if}

	{#if trashView}
		<section class="card stack">
			<div class="row"><h2>Trash</h2><span class="low">Kept for 30 days. Deletes by scripts and agents land here too.</span><span class="spacer"></span><Button size="sm" variant="danger" onclick={emptyTrash}>Empty Trash</Button></div>
			<table>
				<thead><tr><th>Was at</th><th>Deleted</th><th>By</th><th></th></tr></thead>
				<tbody>
					{#each trash as t (t.id)}
						<tr><td>{t.original_path}</td><td class="low mono">{ago(new Date(t.deleted_ms).toISOString())}</td><td class="mid">{#if t.run_id}<a href="/runs/{t.run_id}">{t.deleted_by_label}</a>{:else}{t.deleted_by_label}{/if}</td><td><Button size="sm" onclick={() => restore(t)}>Restore</Button></td></tr>
					{:else}
						<tr><td colspan="4" class="low">Trash is empty.</td></tr>
					{/each}
				</tbody>
			</table>
		</section>
	{:else}
		<div class="split" class:open={!!selected}>
			<section class="card">
				{#if loading && !entries.length}
					<div class="low">Opening your files… your computer wakes if it's asleep.</div>
				{:else}
					<table>
						<thead><tr><th>Name</th><th>Size</th><th>Changed</th><th>From</th></tr></thead>
						<tbody>
							{#each entries as e (e.path)}
								<tr class="click" class:sel={selected?.path === e.path} onclick={() => select(e)} data-testid="file-row">
									<td>{e.is_dir ? '📁' : '📄'} {e.name}</td>
									<td class="mono low">{e.is_dir ? '' : bytes(e.size)}</td>
									<td class="mono low">{ago(new Date(e.modified_ms).toISOString())}</td>
									<td class="low">{#if e.source?.run_id}<a href="/runs/{e.source.run_id}" onclick={(ev) => ev.stopPropagation()}>{e.source.label}</a>{:else}{e.source?.label ?? ''}{/if}</td>
								</tr>
							{:else}
								<tr><td colspan="4"><div class="muted-box">This folder is empty. Upload files or drop them here.</div></td></tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</section>
			{#if selected}
				<aside class="card stack preview">
					<div class="row"><strong>{selected.name}</strong><span class="spacer"></span><button class="x" onclick={() => (selected = null)} aria-label="Close preview">✕</button></div>
					<div class="row">
						<Button size="sm" href={`/api/computers/${session.computerId}/files/download?path=${encodeURIComponent(selected.path)}`}>Download</Button>
						<Button size="sm" onclick={() => ((rename = selected), (newName = selected.name))}>Rename</Button>
						<Button size="sm" variant="danger" onclick={() => remove(selected)}>Delete</Button>
					</div>
					{#if selected.source}<span class="low">{selected.source.label} · {ago(selected.source.at)}</span>{/if}
					{#if !preview}
						<div class="low">Making a preview on your computer…</div>
					{:else if preview.kind === 'table'}
						<div class="tablewrap" data-testid="preview-table">
							<table class="mono"><thead><tr>{#each preview.columns as col, i (i)}<th>{col}</th>{/each}</tr></thead>
								<tbody>{#each preview.rows as r, i (i)}<tr>{#each r as cell, j (j)}<td>{cell}</td>{/each}</tr>{/each}</tbody></table>
						</div>
						<span class="low">Showing {preview.rows.length} of {preview.total_rows} rows</span>
					{:else if preview.kind === 'image'}
						<img src={preview.data_url} alt={selected.name} data-testid="preview-image" />
						<span class="low mono">{preview.original_width} × {preview.original_height}</span>
					{:else if preview.kind === 'text'}
						<pre class="text" data-testid="preview-text">{preview.text}</pre>
						{#if preview.truncated}<span class="low">Showing the start of the file.</span>{/if}
					{:else}
						<div class="muted-box">{preview.reason}</div>
					{/if}
				</aside>
			{/if}
		</div>
	{/if}
</div>

{#if rename}
	<Modal title="Rename" onclose={() => (rename = null)}>
		<input bind:value={newName} aria-label="New name" />
		<div class="row"><Button variant="primary" onclick={doRename}>Rename</Button></div>
	</Modal>
{/if}

<style>
	.files {
		max-width: none;
	}
	.crumb {
		background: none;
		border: 0;
		color: var(--accent-ink);
		cursor: pointer;
		font: inherit;
		padding: 0;
	}
	.split {
		display: grid;
		gap: 16px;
	}
	.split.open {
		grid-template-columns: minmax(0, 1fr) minmax(0, 420px);
	}
	.sel td {
		background: var(--pane);
	}
	.preview {
		align-content: start;
		max-height: 75vh;
		overflow: auto;
	}
	.tablewrap {
		overflow: auto;
		max-height: 420px;
	}
	.text {
		white-space: pre-wrap;
		margin: 0;
		background: var(--pane);
		padding: 10px;
		border-radius: 8px;
		max-height: 420px;
		overflow: auto;
	}
	img {
		max-width: 100%;
		border-radius: 8px;
	}
	.x {
		background: none;
		border: 0;
		color: var(--mid);
		cursor: pointer;
	}
	.usage {
		display: grid;
		gap: 4px;
	}
	.bar {
		height: 6px;
		border-radius: 3px;
		background: var(--pane);
		display: flex;
		overflow: hidden;
	}
	.bar .f {
		background: var(--chart-2);
	}
	.bar .t {
		background: var(--chart-3);
	}
	.drop {
		border: 2px dashed var(--accent);
		border-radius: 12px;
		padding: 24px;
		text-align: center;
		color: var(--accent-ink);
	}
	@media (max-width: 900px) {
		.split.open {
			grid-template-columns: 1fr;
		}
	}
</style>
