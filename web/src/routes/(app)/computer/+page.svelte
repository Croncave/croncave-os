<script lang="ts">
	import { goto } from '$app/navigation';
	import { get, patch, post, del, message } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { refreshMe, session } from '$lib/session.svelte';
	import { ago, bytes, money } from '$lib/format';
	import Status from '$lib/ui/Status.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let c = $state<any>(null);
	let name = $state('');
	let confirm = $state<'' | 'reset' | 'delete'>('');
	let showDetails = $state(false);

	async function load() {
		if (!session.computerId) return;
		c = await get(`/computers/${session.computerId}`);
		name = c.name;
	}
	$effect(() => {
		void session.computerId;
		load();
	});
	$effect(() => onLive(throttle(load, 500)));

	async function save(body: Record<string, unknown>, done = 'Saved') {
		try {
			c = { ...c, ...(await patch(`/computers/${c.id}`, body)) };
			toast(done);
			refreshMe();
		} catch (e) {
			toast(message(e), true);
			load();
		}
	}
	async function action(a: string) {
		try {
			await post(`/computers/${c.id}/action`, { action: a });
			confirm = '';
			toast({ wake: 'Waking up', sleep: 'Going to sleep', restart: 'Restarting', reset: 'Reset with an empty disk' }[a] ?? 'Done');
			refreshMe();
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function remove() {
		try {
			await del(`/computers/${c.id}`);
			await refreshMe();
			goto(session.me?.computers.length ? '/home' : '/computers/new');
		} catch (e) {
			toast(message(e), true);
		}
	}
</script>

<div class="page">
	{#if c}
		<div class="page-head">
			<div class="stack" style="gap: 4px"><h1>{c.name}</h1><Status status={c.status} word={c.status_word} /></div>
			<div class="row">
				{#if c.state === 'asleep'}<Button onclick={() => action('wake')}>Wake</Button>{:else if c.state === 'awake'}<Button onclick={() => action('sleep')}>Sleep now</Button>{/if}
				<Button onclick={() => action('restart')}>Restart</Button>
			</div>
		</div>
		{#if c.note}<div class="banner">{c.note}</div>{/if}
		<section class="card stack">
			<h2>Name</h2>
			<div class="row" style="flex-wrap: nowrap"><input bind:value={name} aria-label="Name" /><Button onclick={() => save({ name })}>Rename</Button></div>
		</section>
		<section class="card stack">
			<h2>Sleep</h2>
			<p class="mid">It sleeps about {c.sleep_delay_secs < 60 ? `${c.sleep_delay_secs} seconds` : `${c.sleep_delay_secs / 60} minutes`} after nothing is active, and stays awake if its next job is due within 5 minutes.</p>
			<Field label="Sleep after nothing is active for">
				<select value={c.sleep_delay_secs} onchange={(e) => save({ sleep_delay_secs: Number(e.currentTarget.value) })}>
					<option value={30}>30 seconds</option>
					<option value={60}>1 minute</option>
					<option value={300}>5 minutes</option>
					<option value={900}>15 minutes</option>
					<option value={3600}>1 hour</option>
				</select>
			</Field>
			<label class="row"><input type="checkbox" checked={c.keep_awake} onchange={(e) => save({ keep_awake: e.currentTarget.checked })} /> Keep it awake all the time
				<span class="low">(about <span class="mono">{money(c.costs.keep_awake_monthly_micros)}</span> a month)</span></label>
		</section>
		<section class="card stack">
			<h2>Size and storage</h2>
			<p class="mid">{c.size[0].toUpperCase() + c.size.slice(1)}: {c.cpu} CPU, {c.memory_gb} GB memory, {c.disk_gb} GB storage. Awake it costs <span class="mono">{money(c.costs.awake_hourly_micros)}</span> an hour.</p>
			<div class="row">
				<select value={c.size} onchange={(e) => save({ size: e.currentTarget.value }, 'Resized')} disabled={c.state !== 'asleep'} aria-label="Size">
					<option value="small">Small</option><option value="medium">Medium</option><option value="large">Large</option>
				</select>
				<Button onclick={() => save({ disk_gb: c.disk_gb + 10 }, 'Added 10 GB')} disabled={c.state !== 'asleep'}>Add 10 GB of storage</Button>
			</div>
			{#if c.state !== 'asleep'}<span class="low">A computer changes size while it's asleep.</span>{/if}
		</section>
		<section class="card stack">
			<button class="toggle" onclick={() => (showDetails = !showDetails)}>{showDetails ? '▾' : '▸'} Details</button>
			{#if showDetails}
				<div class="mono low">Agent {c.agent_version ?? '—'} · connected: {c.connected ? 'yes' : 'no'} · stored {bytes(c.health.disk_bytes)} (+{bytes(c.health.trash_bytes)} in Trash)</div>
				<h3>Recent wakes</h3>
				<div class="list mono">
					{#each c.recent_wakes as w (w.at)}<div class="row"><span>{w.ms} ms</span><span class="low">{w.cause}</span><span class="spacer"></span><span class="low">{ago(w.at)}</span></div>{:else}<div class="low">None yet.</div>{/each}
				</div>
			{/if}
		</section>
		<section class="card stack">
			<h2>Reset or delete</h2>
			<div class="row"><Button variant="danger" onclick={() => (confirm = 'reset')}>Reset…</Button><Button variant="danger" onclick={() => (confirm = 'delete')}>Delete computer…</Button></div>
		</section>
	{/if}
</div>

{#if confirm}
	<Modal title={confirm === 'reset' ? `Reset ${c.name}?` : `Delete ${c.name}?`} onclose={() => (confirm = '')}>
		<p class="mid">{confirm === 'reset' ? 'Its disk is replaced with an empty one. Your files on it, and its Trash, are gone. Jobs stay.' : 'The computer, its files and its jobs are deleted. This cannot be undone.'}</p>
		<div class="row"><Button variant="danger" onclick={() => (confirm === 'reset' ? action('reset') : remove())}>{confirm === 'reset' ? 'Reset' : 'Delete'}</Button><Button onclick={() => (confirm = '')}>Cancel</Button></div>
	</Modal>
{/if}

<style>
	.toggle {
		background: none;
		border: 0;
		color: var(--ink);
		font: inherit;
		font-weight: 600;
		text-align: left;
		cursor: pointer;
		padding: 0;
	}
</style>
