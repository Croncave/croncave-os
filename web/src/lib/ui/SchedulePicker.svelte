<script lang="ts">
	// When a job runs: by hand, on a schedule, or when files change.
	let {
		trigger = $bindable('manual'),
		schedule = $bindable(''),
		watchPath = $bindable(''),
		minSecs = 60
	}: { trigger: string; schedule: string; watchPath: string; minSecs?: number } = $props();

	const choices = [
		{ cron: '* * * * *', label: 'Every minute', secs: 60 },
		{ cron: '*/5 * * * *', label: 'Every 5 minutes', secs: 300 },
		{ cron: '*/15 * * * *', label: 'Every 15 minutes', secs: 900 },
		{ cron: '0 * * * *', label: 'Every hour', secs: 3600 },
		{ cron: '0 */6 * * *', label: 'Every 6 hours', secs: 21600 },
		{ cron: '0 9 * * *', label: 'Every day at 9:00 am', secs: 86400 },
		{ cron: '0 9 * * 1-5', label: 'Weekdays at 9:00 am', secs: 86400 }
	];
	let custom = $state(false);
	$effect(() => {
		if (trigger === 'schedule' && !schedule) schedule = choices.find((c) => c.secs >= minSecs)?.cron ?? '0 * * * *';
	});
	const known = $derived(choices.some((c) => c.cron === schedule || `0 ${c.cron}` === schedule));
</script>

<div class="stack" style="gap: 8px">
	<div class="row" role="radiogroup" aria-label="When it runs">
		<label class="row"><input type="radio" bind:group={trigger} value="manual" /> By hand</label>
		<label class="row"><input type="radio" bind:group={trigger} value="schedule" /> On a schedule</label>
		<label class="row"><input type="radio" bind:group={trigger} value="files" /> When files change</label>
	</div>
	{#if trigger === 'schedule'}
		{#if custom || (!known && schedule)}
			<input class="mono" bind:value={schedule} placeholder="*/15 * * * *" aria-label="Cron schedule" />
			<span class="low">Minute, hour, day of month, month, day of week, in the computer's time zone.</span>
		{:else}
			<select bind:value={schedule} aria-label="Schedule">
				{#each choices as c (c.cron)}
					<option value={c.cron} disabled={c.secs < minSecs}>{c.label}{c.secs < minSecs ? ' (bigger plan)' : ''}</option>
				{/each}
			</select>
		{/if}
		<button class="linkish" type="button" onclick={() => (custom = !custom)}>{custom ? 'Choose from a list' : 'Write a custom schedule'}</button>
	{:else if trigger === 'files'}
		<input bind:value={watchPath} placeholder="Inbox" aria-label="Folder to watch" />
		<span class="low">Runs when files in this folder are added or changed (by you, an upload or another job).</span>
	{/if}
</div>

<style>
	.linkish {
		background: none;
		border: 0;
		padding: 0;
		color: var(--accent-ink);
		cursor: pointer;
		justify-self: start;
		font: inherit;
	}
</style>
