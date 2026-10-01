<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { get, post, message } from '$lib/api';
	import { session } from '$lib/session.svelte';
	import { money } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// The assistant knows what you're looking at, proposes setups, and never acts by itself:
	// applying a proposal is the same call the form makes, recorded as the assistant's.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let messages = $state<any[]>([]);
	let text = $state('');
	let busy = $state(false);

	$effect(() => {
		get('/assistant').then((r) => (messages = r.messages));
	});

	async function ask(e: SubmitEvent) {
		e.preventDefault();
		if (!text.trim()) return;
		busy = true;
		const q = text;
		text = '';
		messages = [...messages, { role: 'user', text: q, proposals: [] }];
		try {
			const r = await post('/assistant', { message: q, context: { app: page.url.pathname.split('/')[1] || 'home', computer: session.computerId } });
			messages = [...messages, { role: 'assistant', text: r.reply, proposals: r.proposals, cost_micros: r.cost_micros }];
		} catch (err) {
			toast(message(err), true);
		} finally {
			busy = false;
		}
	}

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	async function apply(p: any) {
		try {
			const r = await post(
				`/computers/${session.computerId}/watches`,
				{ type_id: p.type_id, name: p.name, inputs: p.inputs, trigger: 'schedule', schedule: p.schedule, check_now: true },
				'assistant'
			);
			p.applied = true;
			messages = [...messages];
			toast('Saved. The first check is running.');
			goto(`/watcher/${r.job.id}`);
		} catch (err) {
			toast(message(err), true);
		}
	}
</script>

<aside class="assistant" aria-label="Assistant">
	<div class="row"><h2>Assistant</h2><span class="spacer"></span><button class="x" onclick={() => (session.assistantOpen = false)} aria-label="Close">✕</button></div>
	<div class="msgs">
		{#each messages as m, i (i)}
			<div class="msg {m.role}">
				<div>{m.text}</div>
				{#each m.proposals ?? [] as p, j (j)}
					<div class="proposal pane stack" style="gap: 6px">
						<strong>{p.title}</strong>
						<span class="mid mono small">{Object.entries(p.inputs ?? {}).map(([k, v]) => `${k}: ${v}`).join(' · ')}</span>
						<div class="row"><Button size="sm" variant="primary" disabled={p.applied} onclick={() => apply(p)}>{p.applied ? 'Applied' : 'Apply'}</Button><span class="low">Nothing runs until you apply it.</span></div>
					</div>
				{/each}
				{#if m.cost_micros}<div class="low mono small">{money(m.cost_micros)}</div>{/if}
			</div>
		{:else}
			<div class="muted-box">Ask me to set up a watch, like "tell me when ACME goes below $90".</div>
		{/each}
	</div>
	<form class="row" onsubmit={ask} style="flex-wrap: nowrap"><input bind:value={text} placeholder="Ask the assistant" aria-label="Ask the assistant" /><Button type="submit" variant="primary" {busy}>Ask</Button></form>
</aside>

<style>
	.assistant {
		position: fixed;
		right: 0;
		top: 52px;
		bottom: 0;
		width: min(400px, 100vw);
		background: var(--surface);
		border-left: 1px solid var(--line);
		padding: 14px;
		display: grid;
		grid-template-rows: auto 1fr auto;
		gap: 10px;
		z-index: 26;
	}
	.msgs {
		overflow: auto;
		display: grid;
		gap: 10px;
		align-content: start;
	}
	.msg {
		display: grid;
		gap: 6px;
		padding: 10px;
		border-radius: 10px;
	}
	.user {
		background: var(--pane);
		justify-self: end;
	}
	.assistant .msg.assistant {
		border: 1px solid var(--line);
	}
	.small {
		font-size: 11.5px;
	}
	.x {
		background: none;
		border: 0;
		color: var(--mid);
		cursor: pointer;
	}
</style>
