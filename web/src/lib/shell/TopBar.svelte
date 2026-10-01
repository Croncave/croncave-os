<script lang="ts">
	import { goto } from '$app/navigation';
	import Status from '$lib/ui/Status.svelte';
	import { session, setComputer, currentComputer } from '$lib/session.svelte';
	import { bytes, money } from '$lib/format';
	import { post } from '$lib/api';
	import { disconnectLive } from '$lib/live';

	let switcher = $state(false);
	let account = $state(false);
	const me = $derived(session.me);
	const c = $derived(currentComputer());

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
</script>

<header class="top">
	<a class="brand" href="/home" aria-label="Home"><span class="mark"></span>Croncave</a>
	<div class="switch">
		<button class="pick" onclick={() => (switcher = !switcher)} aria-haspopup="menu" aria-expanded={switcher} data-testid="computer-switcher">
			{#if c}<span class="name">{c.name}</span><Status status={c.status} word={c.status_word} />{:else}<span class="low">No computer yet</span>{/if}
			<span class="low">▾</span>
		</button>
		{#if c && c.state === 'awake'}
			<span class="vitals mono low" title="CPU, memory and disk on this computer">
				CPU {Math.round(c.health.cpu_percent ?? 0)}% · {c.health.memory_mb ?? 0} MB · {bytes(c.health.disk_bytes)} of {c.disk_gb} GB
			</span>
		{/if}
		{#if switcher}
			<div class="menu card" role="menu">
				{#each me?.computers ?? [] as k (k.id)}
					<button role="menuitem" class="item" onclick={() => choose(k.id)}>
						<span>{k.name} <span class="low">{k.size}</span></span><Status status={k.status} word={k.status_word} />
					</button>
				{/each}
				<a class="item" href="/computers/new" onclick={() => (switcher = false)}>+ New computer</a>
			</div>
		{/if}
	</div>
	<span class="spacer"></span>
	{#if me?.usage}
		<a class="usage" href="/plans" title="Usage left this month, including your award">
			<span class="mono">{money(me.usage.left_micros)}</span> <span class="low">left</span>
			{#if me.account.paused_at}<span class="chip paused">Paused at cap</span>{/if}
		</a>
	{/if}
	{#if me?.account.ai_enabled}
		<button class="icon" onclick={() => (session.assistantOpen = !session.assistantOpen)} aria-label="Ask the assistant">✦ Ask</button>
	{/if}
	<button class="icon bell" onclick={() => (session.activityOpen = !session.activityOpen)} aria-label="Activity" data-testid="bell">
		🔔{#if (me?.unread ?? 0) > 0}<span class="count mono" data-testid="unread">{me?.unread}</span>{/if}
	</button>
	<div class="acct">
		<button class="icon" onclick={() => (account = !account)} aria-haspopup="menu" aria-expanded={account}>{me?.user.name || me?.user.email}</button>
		{#if account}
			<div class="menu card right" role="menu">
				<a class="item" href="/settings" onclick={() => (account = false)}>Settings</a>
				<a class="item" href="/plans" onclick={() => (account = false)}>Plans and billing</a>
				<a class="item" href="/usage" onclick={() => (account = false)}>Usage</a>
				{#if me?.user.is_admin}<a class="item" href="/admin" onclick={() => (account = false)}>Admin</a>{/if}
				{#if me?.dev_tools}<a class="item" href="/dev" onclick={() => (account = false)}>Dev tools</a>{/if}
				<button class="item" onclick={signOut}>Sign out</button>
			</div>
		{/if}
	</div>
</header>

<style>
	.top {
		height: 52px;
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 0 14px;
		border-bottom: 1px solid var(--line);
		background: var(--surface);
		position: sticky;
		top: 0;
		z-index: 20;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 600;
		color: var(--ink);
	}
	.mark {
		width: 16px;
		height: 16px;
		border-radius: 5px;
		background: var(--accent);
	}
	.switch {
		position: relative;
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.pick {
		display: flex;
		align-items: center;
		gap: 10px;
		background: var(--pane);
		border: 1px solid var(--line);
		border-radius: 8px;
		padding: 5px 10px;
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.name {
		font-weight: 500;
	}
	.vitals {
		font-size: 11.5px;
	}
	.menu {
		position: absolute;
		top: 42px;
		left: 0;
		min-width: 280px;
		padding: 6px;
		display: grid;
		z-index: 30;
	}
	.right {
		left: auto;
		right: 0;
		min-width: 200px;
	}
	.item {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 8px;
		border-radius: 6px;
		background: none;
		border: 0;
		color: var(--ink);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.item:hover {
		background: var(--pane);
		text-decoration: none;
	}
	.usage {
		color: var(--ink);
	}
	.paused {
		color: var(--needs);
	}
	.icon {
		background: none;
		border: 1px solid transparent;
		color: var(--ink);
		font: inherit;
		cursor: pointer;
		padding: 5px 8px;
		border-radius: 8px;
		position: relative;
	}
	.icon:hover {
		background: var(--pane);
	}
	.acct {
		position: relative;
	}
	.count {
		position: absolute;
		top: -2px;
		right: -4px;
		background: var(--needs);
		color: var(--bg);
		border-radius: 999px;
		font-size: 10px;
		padding: 0 5px;
	}
	@media (max-width: 760px) {
		.vitals {
			display: none;
		}
	}
</style>
