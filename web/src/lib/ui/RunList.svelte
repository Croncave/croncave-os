<script lang="ts">
	import Status from './Status.svelte';
	import { ago } from '$lib/format';
	import type { RunBrief } from '$lib/types';
	let { runs, empty = 'Nothing has run yet.' }: { runs: RunBrief[]; empty?: string } = $props();
	const trigger = (t: string) => ({ schedule: 'on schedule', manual: 'by you', test: 'test', retry: 'retry', files: 'files changed' })[t] ?? t;
</script>

{#if runs.length === 0}
	<div class="muted-box">{empty}</div>
{:else}
	<table>
		<thead><tr><th>Result</th><th></th><th>Started</th><th>Why</th></tr></thead>
		<tbody>
			{#each runs as r (r.id)}
				<tr class="click" onclick={() => (location.href = `/runs/${r.id}`)}>
					<td><Status status={r.data?.couldnt_check ? 'couldnt_check' : r.status} /></td>
					<td><a href="/runs/{r.id}">{r.headline ?? r.status_note ?? ''}</a></td>
					<td class="mono low">{ago(r.started_at ?? r.queued_at)}</td>
					<td class="low">{trigger(r.trigger)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/if}
