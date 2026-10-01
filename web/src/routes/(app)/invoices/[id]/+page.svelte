<script lang="ts">
	import { page } from '$app/state';
	import { get } from '$lib/api';
	import { money, when } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let inv = $state<any>(null);
	$effect(() => {
		get(`/billing/invoices/${page.params.id}`).then((r) => (inv = r));
	});
</script>

<div class="page">
	{#if inv}
		<div class="row"><a href="/plans" class="low">← Plans and billing</a><span class="spacer"></span><Button onclick={() => print()}>Download (print to PDF)</Button></div>
		<section class="card stack invoice">
			<div class="row"><h1>Invoice {inv.number}</h1><span class="spacer"></span><span class="chip">{inv.status}</span></div>
			<div class="mid">{inv.account} · {inv.email}</div>
			<div class="low">Issued {when(inv.created_at)} · {inv.card ?? ''}</div>
			<table>
				<tbody>
					{#each inv.lines as l, i (i)}<tr><td>{l.label}</td><td class="mono right">{money(l.amount_micros)}</td></tr>{/each}
					<tr><td><strong>Total</strong></td><td class="mono right"><strong>{money(inv.total_micros)}</strong></td></tr>
				</tbody>
			</table>
			<div class="low mono">Payment reference {inv.reference ?? '—'} (test mode)</div>
		</section>
	{/if}
</div>

<style>
	.right {
		text-align: right;
	}
</style>
