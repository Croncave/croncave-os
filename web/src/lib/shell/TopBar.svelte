<script lang="ts">
	import { goto } from '$app/navigation';
	import Status from '$lib/ui/Status.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Mark from './Mark.svelte';
	import { session, setComputer, currentComputer } from '$lib/session.svelte';
	import { money } from '$lib/format';
	import { post } from '$lib/api';
	import { disconnectLive } from '$lib/live';
	import type { Computer } from '$lib/types';

	let switcher = $state(false);
	let account = $state(false);
	const me = $derived(session.me);
	const c = $derived(currentComputer());

	const initials = $derived.by(() => {
		const n = (me?.user.name || me?.user.email || '?').trim();
		const parts = n.split(/[\s@._-]+/).filter(Boolean);
		return ((parts[0]?.[0] ?? '?') + (parts[1]?.[0] ?? '')).toUpperCase();
	});

	// Vitals only mean something while the computer is awake.
	const vitals = $derived.by(() => {
		if (!c || c.state !== 'awake') return null;
		const cpu = Math.round(c.health.cpu_percent ?? 0);
		const memGb = (c.health.memory_mb ?? 0) / 1024;
		const diskGb = (c.health.disk_bytes ?? 0) / 1e9;
		return [
			{ label: 'CPU', pct: cpu, value: `${cpu}%` },
			{ label: 'RAM', pct: (memGb / c.memory_gb) * 100, value: `${gb(memGb)}/${c.memory_gb} GB` },
			{ label: 'DISK', pct: (diskGb / c.disk_gb) * 100, value: `${gb(diskGb)}/${c.disk_gb} GB` }
		];
	});
	function gb(n: number) {
		return n >= 10 ? Math.round(n).toString() : n.toFixed(1);
	}

	// Usage left, out of what this month started with (award, allowance, credits).
	const usage = $derived.by(() => {
		const u = me?.usage;
		if (!u) return null;
		const total = u.left_micros + u.spent_this_month_micros;
		return { name: u.plan_name, left: u.left_micros, pct: total > 0 ? (u.left_micros / total) * 100 : 0 };
	});

	function spec(k: Computer) {
		return `${k.size[0].toUpperCase()}${k.size.slice(1)} · ${k.cpu} CPU · ${k.memory_gb} GB`;
	}
	function doing(k: Computer) {
		if (k.state === 'asleep') return 'Wakes in a few seconds when opened';
		const bits = [];
		if (k.running) bits.push(`${k.running} job${k.running === 1 ? '' : 's'} running`);
		if (k.next_job) bits.push(`next: ${k.next_job.name}`);
		return bits.join(' · ') || 'Nothing running';
	}

	async function signOut() {
		await post('/auth/signout');
		disconnectLive();
		session.me = null;
		goto('/signin');
	}
	function choose(id: string) {
		setComputer(id);
		switcher = false;
	}
	function closeMenus(e: MouseEvent) {
		const t = e.target as HTMLElement;
		if (!t.closest('.switch')) switcher = false;
		if (!t.closest('.acct')) account = false;
	}
</script>

<svelte:window onclick={closeMenus} onkeydown={(e) => e.key === 'Escape' && ((switcher = false), (account = false))} />

<header class="top">
	<div class="switch">
		<button class="pick" onclick={() => (switcher = !switcher)} aria-haspopup="menu" aria-expanded={switcher} data-testid="computer-switcher">
			<Mark />
			{#if c}<span class="name">{c.name}</span><Status status={c.status} word={c.status_word} />{:else}<span class="name">Croncave</span>{/if}
			<span class="chev"><Icon name="chevron-down" size={14} /></span>
		</button>
		{#if switcher}
			<div class="menu" role="menu" aria-label="Computers">
				<div class="label head">Your computers</div>
				{#each me?.computers ?? [] as k (k.id)}
					<button role="menuitem" class="comp" class:current={k.id === c?.id} onclick={() => choose(k.id)}>
						<span class="tile"><Icon name="server" size={16} /></span>
						<span class="what">
							<span class="line1"><span class="cname">{k.name}</span><Status status={k.status} word={k.status_word} /></span>
							<span class="mono spec">{spec(k)}</span>
							<span class="doing">{doing(k)}</span>
						</span>
						{#if k.id === c?.id}<span class="tick"><Icon name="check" size={16} stroke={2} /></span>{:else}<span class="tick"></span>{/if}
					</button>
				{/each}
				<div class="rule"></div>
				<a class="comp" href="/computers/new" onclick={() => (switcher = false)}>
					<span class="tile dashed"><Icon name="plus" size={16} /></span><span class="what"><span class="cname plain">New computer</span></span>
				</a>
				<a class="manage" href="/computer" onclick={() => (switcher = false)}>Manage computers</a>
			</div>
		{/if}
	</div>

	<div class="right">
		{#if vitals}
			<div class="vitals" title="CPU, memory and disk on this computer">
				{#each vitals as v (v.label)}
					<div class="vital">
						<span class="vl">{v.label}</span>
						<span class="bar"><span style="width: {Math.min(100, Math.max(v.pct, v.pct > 0 ? 2 : 0))}%"></span></span>
						<span class="vv">{v.value}</span>
					</div>
				{/each}
			</div>
			<span class="divider"></span>
		{/if}
		{#if usage}
			<a class="usage" href="/usage" title="Usage left this month: your award first, then your monthly allowance">
				<span class="ut"><span class="un">{usage.name} · usage left</span><span class="mono uv">{money(usage.left)}</span></span>
				<span class="ubar"><span style="width: {Math.min(100, usage.pct)}%"></span></span>
			</a>
		{/if}
		<div class="acct">
			<button class="avatar" onclick={() => (account = !account)} aria-haspopup="menu" aria-expanded={account} aria-label="Account">{initials}</button>
			{#if account}
				<div class="menu right-menu" role="menu">
					<div class="who"><span class="cname">{me?.user.name || 'You'}</span><span class="mono doing">{me?.user.email}</span></div>
					<div class="rule"></div>
					<a class="item" href="/settings/account" onclick={() => (account = false)}><Icon name="user" />Account</a>
					<a class="item" href="/usage" onclick={() => (account = false)}><Icon name="gauge" />Plan and usage</a>
					<a class="item" href="/settings" onclick={() => (account = false)}><Icon name="settings" />Settings</a>
					{#if me?.user.is_admin}<a class="item" href="/admin" onclick={() => (account = false)}><Icon name="shield" />Admin</a>{/if}
					{#if me?.dev_tools}<a class="item" href="/dev" onclick={() => (account = false)}><Icon name="terminal" />Dev tools</a>{/if}
					<div class="rule"></div>
					<button class="item" onclick={signOut}><Icon name="arrow-right" />Sign out</button>
				</div>
			{/if}
		</div>
	</div>
</header>

<style>
	.top {
		height: 52px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 16px 0 12px;
		background: var(--bg);
		position: sticky;
		top: 0;
		z-index: 20;
	}
	.switch,
	.acct {
		position: relative;
	}
	.pick {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 36px;
		padding: 0 12px 0 8px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.pick:hover {
		border-color: var(--line-strong);
	}
	.name {
		font-size: 14px;
		font-weight: 600;
	}
	.chev {
		color: var(--low);
		display: flex;
	}
	.right {
		display: flex;
		align-items: center;
		gap: 20px;
	}
	.vitals {
		display: flex;
		align-items: center;
		gap: 20px;
	}
	.vital {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.vl {
		font-family: 'JetBrains Mono', ui-monospace, monospace;
		font-size: 11px;
		letter-spacing: 0.06em;
		color: var(--low);
	}
	.bar {
		width: 48px;
		height: 6px;
		border-radius: 3px;
		background: var(--line);
	}
	.bar span {
		display: block;
		height: 6px;
		border-radius: 3px;
		background: var(--ink);
	}
	.vv {
		font-family: 'JetBrains Mono', ui-monospace, monospace;
		font-size: 12px;
		color: var(--ink);
	}
	.divider {
		width: 1px;
		height: 20px;
		background: var(--line);
	}
	.usage {
		display: flex;
		flex-direction: column;
		gap: 5px;
		width: 190px;
		padding: 6px 12px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
	}
	.usage:hover {
		text-decoration: none;
		border-color: var(--line-strong);
	}
	.ut {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
	}
	.un {
		font-size: 12px;
		color: var(--mid);
	}
	.uv {
		font-size: 12px;
		font-weight: 500;
	}
	.ubar {
		height: 4px;
		border-radius: 2px;
		background: var(--line);
	}
	.ubar span {
		display: block;
		height: 4px;
		border-radius: 2px;
		background: var(--accent);
	}
	.avatar {
		width: 30px;
		height: 30px;
		border-radius: 9999px;
		background: var(--accent-soft);
		color: var(--accent-ink);
		border: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		font: 600 12px Inter, system-ui, sans-serif;
		cursor: pointer;
	}
	.menu {
		position: absolute;
		left: 0;
		top: 42px;
		width: 380px;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 12px;
		box-shadow: 0 16px 40px var(--shadow);
		padding: 8px;
		display: flex;
		flex-direction: column;
		gap: 4px;
		z-index: 30;
	}
	.right-menu {
		left: auto;
		right: 0;
		width: 260px;
	}
	.head {
		padding: 6px 12px 4px;
		letter-spacing: 0.08em;
	}
	.comp {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 12px;
		border-radius: 8px;
		background: transparent;
		border: 0;
		color: var(--ink);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.comp:hover,
	.comp.current {
		background: var(--pane);
		text-decoration: none;
	}
	.tile {
		width: 32px;
		height: 32px;
		border-radius: 8px;
		background: var(--raised);
		color: var(--ink);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.dashed {
		background: none;
		border: 1.5px dashed var(--line-strong);
		color: var(--mid);
	}
	.what {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}
	.line1 {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.cname {
		font-size: 14px;
		font-weight: 600;
	}
	.cname.plain {
		font-weight: 500;
	}
	.spec {
		font-size: 12px;
		color: var(--mid);
	}
	.doing {
		font-size: 12px;
		color: var(--low);
	}
	.tick {
		width: 16px;
		color: var(--accent-ink);
		display: flex;
	}
	.rule {
		height: 1px;
		background: var(--line);
		margin: 4px 0;
	}
	.manage {
		padding: 4px 12px 6px;
		font-size: 13px;
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.who {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px 10px 4px;
	}
	.item {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 36px;
		padding: 0 10px;
		border-radius: 6px;
		background: none;
		border: 0;
		color: var(--ink);
		font: 500 14px Inter, system-ui, sans-serif;
		text-align: left;
		cursor: pointer;
	}
	.item:hover {
		background: var(--pane);
		text-decoration: none;
	}
	@media (max-width: 1100px) {
		.vitals,
		.divider {
			display: none;
		}
	}
	@media (max-width: 600px) {
		.usage {
			width: auto;
		}
		.un {
			display: none;
		}
		.menu {
			width: calc(100vw - 24px);
		}
	}
</style>
