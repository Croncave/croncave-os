<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import Button from '$lib/ui/Button.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import ScriptAdd from '$lib/apps/scripts/ScriptAdd.svelte';
	import { loadScripts, scripts } from '$lib/apps/scripts/store.svelte';

	// /scripts opens your first script, or the Add screen with ?add.
	const adding = $derived(page.url.searchParams.has('add'));
	$effect(() => {
		if (!adding && scripts.loaded && scripts.jobs.length) goto(`/scripts/${scripts.jobs[0].id}`, { replaceState: true });
	});
</script>

{#if adding}
	<ScriptAdd onsaved={(j) => (loadScripts(), goto(`/scripts/${j.id}`))} />
{:else if scripts.loaded && !scripts.jobs.length}
	<div class="none">
		<span class="ic"><Icon name="code" size={22} /></span>
		<h2>Run your own scripts</h2>
		<p>Python, Node.js and shell scripts, by hand, on a schedule, or when files change. Your computer wakes up for each run, then goes back to sleep.</p>
		<Button variant="primary" icon="plus" href="/scripts?add=1">Add a script</Button>
	</div>
{/if}

<style>
	.none {
		margin: auto;
		max-width: 420px;
		text-align: center;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		padding: 40px 20px;
	}
	.ic {
		width: 48px;
		height: 48px;
		border-radius: 12px;
		background: var(--raised);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	h2 {
		font-size: 20px;
		margin: 6px 0 0;
	}
	p {
		color: var(--mid);
		margin: 0 0 8px;
	}
</style>
