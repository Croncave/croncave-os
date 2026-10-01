<script lang="ts">
	import { goto } from '$app/navigation';
	import { get, post, message } from '$lib/api';
	import { onChange, throttle } from '$lib/live';
	import { currentComputer, session, setComputer } from '$lib/session.svelte';
	import { appIcon, appName, clock, greeting } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// Home: what happened since you left, what needs you, and what's still running.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let home = $state<any>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let jobs = $state<any[]>([]);
	let q = $state('');
	let active = $state(0);
	let search: HTMLInputElement;

	async function load() {
		home = await get('/home');
		if (session.computerId) jobs = (await get(`/computers/${session.computerId}/jobs`)).jobs;
	}
	$effect(() => {
		void session.computerId;
		load();
	});
	$effect(() => onChange(throttle(load, 400)));

	const c = $derived(currentComputer());
	const first = $derived.by(() => {
		const n = session.me?.user.name ?? '';
		return n && !n.startsWith('User-') ? n.split(' ')[0] : '';
	});
	const count = (app: string) => home?.active.filter((a: { computer_id: string; app: string }) => a.computer_id === c?.id && a.app === app).reduce((s: number, a: { count: number }) => s + a.count, 0) ?? 0;

	type Row = { key: string; icon: string; title: string; sub: string; at: string; needs?: boolean; href?: string; progress?: number | null; approval?: { run_id: string; request_id: string } };
	const rows = $derived.by(() => {
		if (!home) return [] as Row[];
		const out: Row[] = [];
		for (const a of home.needs_you.approvals)
			out.push({ key: `a${a.request_id}`, icon: 'braces', title: `${a.job} is waiting for you`, sub: `Wants to run ${a.command}`, at: '', needs: true, href: `/code?run=${a.run_id}`, approval: a });
		for (const r of home.needs_you.reviews) out.push({ key: `r${r.run_id}`, icon: 'braces', title: r.title, sub: 'Review its changes', at: '', needs: true, href: `/code?review=${r.run_id}` });
		for (const f of home.needs_you.failing)
			out.push({ key: `f${f.job_id}`, icon: appIcon(f.app), title: `${f.name} didn't finish`, sub: f.fix ?? f.why ?? 'See what went wrong', at: '', needs: true, href: `/runs/${f.run_id}` });
		for (const r of home.running) {
			const p = r.progress?.total ? r.progress.done / r.progress.total : null;
			out.push({ key: `p${r.run_id}`, icon: appIcon(r.app), title: `${r.job} is still running`, sub: r.progress?.message ?? `${appName(r.app)} · started ${clock(r.started_at)}`, at: 'now', href: `/runs/${r.run_id}`, progress: p });
		}
		const failing = new Set(home.needs_you.failing.map((f: { job_id: string }) => f.job_id));
		for (const e of home.happened) {
			// Results only; setting things up is in Activity.
			if (e.level === 'info' || (e.level === 'failed' && failing.has(e.job_id))) continue;
			out.push({
				key: `e${e.id}`,
				icon: appIcon(e.app),
				title: e.title,
				sub: e.body?.split('\n')[0] || appName(e.app),
				at: clock(e.created_at),
				needs: e.level === 'needs_you',
				href: e.run_id ? `/runs/${e.run_id}` : e.job_id && e.app !== 'billing' ? `/${e.app}/${e.job_id}` : e.app === 'billing' ? '/usage' : undefined
			});
		}
		return out.slice(0, 9);
	});
	const fresh = $derived(home && !rows.length && !jobs.length);

	// Search: apps, this computer's jobs, and a few commands.
	const places = [
		{ label: 'Files', sub: 'App', href: '/files', icon: 'folder' },
		{ label: 'Scripts', sub: 'App', href: '/scripts', icon: 'code' },
		{ label: 'Watcher', sub: 'App', href: '/watcher', icon: 'eye' },
		{ label: 'Code', sub: 'App', href: '/code', icon: 'braces' },
		{ label: 'New watch', sub: 'Command', href: '/watcher/new', icon: 'plus' },
		{ label: 'Add a script', sub: 'Command', href: '/scripts?add=1', icon: 'plus' },
		{ label: 'Upload files', sub: 'Command', href: '/files', icon: 'upload' },
		{ label: 'New computer', sub: 'Command', href: '/computers/new', icon: 'server' },
		{ label: 'Plan and usage', sub: 'Settings', href: '/usage', icon: 'gauge' },
		{ label: 'Settings', sub: 'Settings', href: '/settings', icon: 'settings' }
	];
	const matches = $derived.by(() => {
		const t = q.trim().toLowerCase();
		if (!t) return [];
		return [
			...places.filter((p) => p.label.toLowerCase().includes(t)),
			...jobs.filter((j) => j.name.toLowerCase().includes(t)).map((j) => ({ label: j.name, sub: appName(j.app), href: `/${j.app}/${j.id}`, icon: appIcon(j.app) }))
		].slice(0, 8);
	});
	$effect(() => {
		void q;
		active = 0;
	});

	function keys(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') (e.preventDefault(), (active = Math.min(matches.length - 1, active + 1)));
		else if (e.key === 'ArrowUp') (e.preventDefault(), (active = Math.max(0, active - 1)));
		else if (e.key === 'Escape') q = '';
	}
	function submit(e: SubmitEvent) {
		e.preventDefault();
		if (matches[active]) return goto(matches[active].href);
		if (q.trim()) {
			session.assistantOpen = true;
			if (!session.me?.account.ai_enabled) toast('Nothing matches. The assistant can answer in your own words once it’s on.');
		}
	}
	function shortcut(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
			e.preventDefault();
			search?.focus();
		}
	}

	async function approve(a: { run_id: string; request_id: string }, ok: boolean) {
		try {
			await post(`/runs/${a.run_id}/approvals/${a.request_id}`, { approved: ok });
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	function open(r: Row) {
		if (!r.href) return;
		const h = home?.needs_you.failing.find((f: { run_id: string }) => r.href?.endsWith(f.run_id));
		if (h) setComputer(h.computer_id);
		goto(r.href);
	}
	const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
</script>

<svelte:window onkeydown={shortcut} />

<div class="home">
	<div class="col">
		<div class="hello">
			<h1>{greeting(fresh ? first : '')}</h1>
			{#if c && !fresh}
				<p data-testid="since">
					{c.name} is {c.status_word.toLowerCase()}.
					<strong>{c.running}</strong> job{c.running === 1 ? '' : 's'} running, <strong>{count('watcher')}</strong> watch{count('watcher') === 1 ? '' : 'es'} on.
				</p>
			{/if}
		</div>

		<form class="search" onsubmit={submit} role="search">
			<label class="box">
				<span class="ic"><Icon name="search" size={20} /></span>
				<input bind:this={search} bind:value={q} onkeydown={keys} placeholder="Search files and apps, or type a command" aria-label="Search or run a command" autocomplete="off" />
				<kbd class="mono">{mac ? '⌘K' : 'Ctrl K'}</kbd>
			</label>
			{#if matches.length}
				<div class="results" role="listbox" aria-label="Results">
					{#each matches as m, i (m.href + m.label)}
						<a href={m.href} class="res" class:on={i === active} role="option" aria-selected={i === active} onmouseenter={() => (active = i)}>
							<span class="rt"><Icon name={m.icon} size={15} /></span><span>{m.label}</span><span class="grow"></span><span class="low">{m.sub}</span>
						</a>
					{/each}
				</div>
			{/if}
		</form>

		{#if fresh}
			<div class="section">
				<span class="sh">Get started</span>
				<div class="rows">
					<a class="row" href="/scripts?add=1">
						<span class="tile"><Icon name="code" size={18} stroke={2} /></span>
						<span class="what"><span class="t">Run your first script</span><span class="s">Upload a .py, .js or .sh file, or start from an example</span></span>
					</a>
					<a class="row" href="/files">
						<span class="tile"><Icon name="folder" size={18} stroke={2} /></span>
						<span class="what"><span class="t">Bring in your files</span><span class="s">Upload files or a folder. They stay here even while the computer sleeps</span></span>
					</a>
					<div class="row">
						<span class="tile"><Icon name="apps" size={18} stroke={2} /></span>
						<span class="what"><span class="t">Add more apps</span><span class="s">Watcher, Code and more, ready in a click</span></span>
						<Button href="/apps">Browse apps</Button>
					</div>
				</div>
			</div>
		{:else if home}
			<div class="section">
				<div class="sh-row"><span class="sh">Since you left</span><button class="open" onclick={() => (session.activityOpen = true)}>Open Activity</button></div>
				<div class="rows">
					{#each rows as r (r.key)}
						{#snippet body()}
							<span class="tile"><Icon name={r.icon} size={18} stroke={2} /></span>
							<span class="what">
								<span class="t">{r.title}{#if r.needs}<span class="pill"><span class="dot"></span>Needs you</span>{/if}</span>
								<span class="s">{r.sub}</span>
								{#if r.progress != null}<span class="bar"><span style="width: {Math.round(r.progress * 100)}%"></span></span>{/if}
							</span>
						{/snippet}
						{#if r.approval}
							<div class="row" data-testid="needs-you">
								{@render body()}
								<span class="acts">
									<Button size="sm" variant="primary" onclick={() => approve(r.approval!, true)}>Approve</Button>
									<Button size="sm" onclick={() => approve(r.approval!, false)}>Deny</Button>
								</span>
							</div>
						{:else if r.href}
							<a class="row" href={r.href} onclick={(e) => (e.preventDefault(), open(r))} data-testid={r.needs ? 'needs-you' : undefined}>
								{@render body()}<span class="at mono">{r.at}</span>
							</a>
						{:else}
							<div class="row" data-testid={r.needs ? 'needs-you' : undefined}>{@render body()}<span class="at mono">{r.at}</span></div>
						{/if}
					{:else}
						<div class="row quiet"><span class="s">Nothing new since you left. Your watches and scripts keep going while you're away.</span></div>
					{/each}
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	.home {
		flex: 1;
		display: flex;
		justify-content: center;
		align-items: flex-start;
		padding: max(32px, 18vh) 32px 120px;
	}
	.col {
		width: 840px;
		max-width: 100%;
		display: flex;
		flex-direction: column;
		gap: 28px;
	}
	.hello {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	h1 {
		font-size: 28px;
		line-height: 36px;
		font-weight: 600;
		letter-spacing: -0.02em;
	}
	.hello p {
		font-size: 14px;
		color: var(--mid);
	}
	.hello strong {
		color: var(--ink);
		font-weight: 600;
	}
	.search {
		position: relative;
	}
	.box {
		display: flex;
		align-items: center;
		gap: 12px;
		height: 56px;
		padding: 0 14px 0 18px;
		border-radius: 12px;
		border: 1px solid var(--line-strong);
		background: var(--surface);
	}
	.box:focus-within {
		border-color: var(--working);
	}
	.ic {
		color: var(--low);
		display: flex;
	}
	.box input {
		flex: 1;
		height: auto;
		border: 0;
		padding: 0;
		background: transparent;
		font-size: 16px;
		box-shadow: none;
	}
	kbd {
		font-size: 12px;
		padding: 2px 6px;
		border-radius: 4px;
		border: 1px solid var(--line);
		background: var(--pane);
		color: var(--mid);
	}
	.results {
		position: absolute;
		left: 0;
		right: 0;
		top: 62px;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 12px;
		box-shadow: 0 16px 40px var(--shadow);
		padding: 6px;
		display: flex;
		flex-direction: column;
		z-index: 10;
	}
	.res {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 9px 12px;
		border-radius: 8px;
		color: var(--ink);
		font-size: 14px;
	}
	.res.on {
		background: var(--raised);
		text-decoration: none;
	}
	.rt {
		color: var(--mid);
		display: flex;
	}
	.grow {
		flex: 1;
	}
	.section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.sh-row {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
	}
	.sh {
		font-size: 15px;
		font-weight: 600;
	}
	.open {
		background: none;
		border: 0;
		padding: 0;
		font: 13px Inter, system-ui, sans-serif;
		color: var(--accent-ink);
		text-decoration: underline;
		text-underline-offset: 3px;
		cursor: pointer;
	}
	.rows {
		border-radius: 12px;
		border: 1px solid var(--line);
		background: var(--surface);
		overflow: hidden;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 14px 18px;
		color: var(--ink);
	}
	.row + .row {
		border-top: 1px solid var(--line);
	}
	a.row:hover {
		background: var(--pane);
		text-decoration: none;
		cursor: pointer;
	}
	.tile {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		background: var(--raised);
		color: var(--ink);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.what {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.t {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 15px;
		font-weight: 500;
	}
	.s {
		font-size: 13px;
		color: var(--mid);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.quiet .s {
		white-space: normal;
	}
	.at {
		font-size: 12px;
		color: var(--low);
		min-width: 44px;
		text-align: right;
	}
	.acts {
		display: flex;
		gap: 6px;
	}
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 22px;
		padding: 0 8px;
		border-radius: 4px;
		background: var(--needs-soft);
		color: var(--needs);
		font-size: 12px;
		font-weight: 600;
		flex-shrink: 0;
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--needs);
	}
	.bar {
		width: 180px;
		height: 4px;
		border-radius: 2px;
		background: var(--line);
		margin-top: 6px;
	}
	.bar span {
		display: block;
		height: 4px;
		border-radius: 2px;
		background: var(--working);
	}
	@media (max-width: 760px) {
		.home {
			padding: 24px 12px 120px;
		}
	}
</style>
