<script lang="ts">
	// Limits on a job: longest run, retries, overlap, what it may spend, when to be told.
	let {
		maxRuntime = $bindable(600),
		retries = $bindable(0),
		overlap = $bindable('skip'),
		notifyFinished = $bindable(false),
		notifyFailed = $bindable(true)
	}: { maxRuntime: number; retries: number; overlap: string; notifyFinished: boolean; notifyFailed: boolean } = $props();
</script>

<div class="grid2">
	<label class="stack" style="gap: 4px"><span>Longest run</span>
		<select bind:value={maxRuntime}>
			<option value={60}>1 minute</option>
			<option value={300}>5 minutes</option>
			<option value={600}>10 minutes</option>
			<option value={1800}>30 minutes</option>
			<option value={3600}>1 hour</option>
			<option value={14400}>4 hours</option>
		</select>
	</label>
	<label class="stack" style="gap: 4px"><span>If it fails, retry</span>
		<select bind:value={retries}>
			<option value={0}>Don't retry</option>
			<option value={1}>Once</option>
			<option value={3}>Up to 3 times</option>
		</select>
	</label>
	<label class="stack" style="gap: 4px"><span>If the last run is still going</span>
		<select bind:value={overlap}>
			<option value="skip">Skip this run</option>
			<option value="queue">Run it after</option>
		</select>
	</label>
	<div class="stack" style="gap: 4px"><span>Tell me</span>
		<label class="row"><input type="checkbox" bind:checked={notifyFailed} /> When it fails</label>
		<label class="row"><input type="checkbox" bind:checked={notifyFinished} /> Every time it finishes</label>
	</div>
</div>
