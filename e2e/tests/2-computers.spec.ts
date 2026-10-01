import { expect, test } from '@playwright/test';
import { api, newComputer, signUp } from './helpers';

test('a computer wakes, sleeps about 30 seconds after nothing is active, and wakes for work', async ({ page }) => {
	test.setTimeout(180_000);
	await signUp(page, { plan: 'free' });
	await page.goto('/computers/new');
	// Free can't make a Medium computer.
	await expect(page.locator('[data-size=medium]')).toBeDisabled();
	await newComputer(page, 'Night owl');

	// Home doesn't read the computer, so it doesn't keep it awake.
	const switcher = page.getByTestId('computer-switcher');
	await expect(switcher).toContainText('Asleep', { timeout: 75_000 });

	// Opening Files wakes it on demand.
	await page.goto('/files');
	await expect(switcher).toContainText(/Waking up|Awake/);
	await expect(switcher).toContainText('Awake');
	await expect(page.getByRole('heading', { name: "Your computer's files live here" })).toBeVisible();

	// Put it to sleep, then a scheduled job wakes it: move time to the job's slot.
	const me = await api(page, 'GET', '/me');
	const computer = me.computers[0].id;
	await page.goto('/computer');
	await page.getByRole('button', { name: 'Sleep now' }).click();
	await expect(switcher).toContainText('Asleep');
	await api(page, 'POST', `/computers/${computer}/scripts/template`, { template: 'backup' });
	await page.getByRole('button', { name: 'Sleep now' }).click();
	await expect(switcher).toContainText('Asleep');
	const made = await api(page, 'POST', `/computers/${computer}/scripts`, {
		name: 'Hourly backup',
		path: 'Scripts/backup/backup.sh',
		trigger: 'schedule',
		schedule: '0 * * * *'
	});
	const due = new Date(made.job.next_due_at).getTime() - new Date(me.now).getTime();
	await api(page, 'POST', '/dev/clock', { secs: Math.ceil(due / 1000) + 1 });
	await page.goto(`/scripts/${made.job.id}`);
	await expect(page.locator('table')).toContainText('Backed up Scripts', { timeout: 30_000 });
	await expect(page.locator('table')).toContainText('on schedule');

	// The details layer shows the measured wake times.
	await page.goto('/computer');
	await page.getByRole('button', { name: /Details/ }).click();
	await expect(page.getByText(/\d+ ms/).first()).toBeVisible();
});
