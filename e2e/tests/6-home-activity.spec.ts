import { expect, test } from '@playwright/test';
import { api, newComputer, signUp } from './helpers';

test('Home and Activity show what happened, unread notifications, and update without refreshing', async ({ page }) => {
	await signUp(page);
	await newComputer(page, 'Home base');
	const me = await api(page, 'GET', '/me');
	const computer = me.computers[0].id;
	await page.goto('/home');
	await expect(page.getByRole('heading', { name: /Good (morning|afternoon|evening)/ })).toBeVisible();

	// Something happens elsewhere: the open Home page updates by itself.
	await api(page, 'POST', `/computers/${computer}/scripts/template`, { template: 'csv-report' });
	await api(page, 'POST', `/computers/${computer}/scripts`, { name: 'Live report', path: 'Scripts/csv-report/report.py', run_now: true });
	await expect(page.getByText('Live report: 10 sales summarized').first()).toBeVisible();

	// A failure needs you: it shows on Home with why, and in Activity as unread.
	await page.request.put(`/api/computers/${computer}/files/write?path=Scripts/bad.sh`, { data: 'exit 3\n' });
	await api(page, 'POST', `/computers/${computer}/scripts`, { name: 'Broken job', path: 'Scripts/bad.sh', run_now: true });
	await expect(page.getByRole('heading', { name: 'Needs you' })).toBeVisible();
	await expect(page.getByText('Broken job').first()).toBeVisible();
	await expect(page.getByTestId('unread')).toBeVisible();

	await page.getByTestId('bell').click();
	const panel = page.getByTestId('activity');
	await expect(panel).toContainText('"Broken job" failed');
	await panel.getByRole('tab', { name: /Unread/ }).click();
	await expect(panel).toContainText('Set up "Live report"');
	await panel.getByRole('button', { name: 'Mark all read' }).click();
	await expect(panel).toContainText("You're all caught up.");
	await expect(page.getByTestId('unread')).toHaveCount(0);

	// Notifications went out by email (the outbox).
	// Delivery is asynchronous (the notifier runs every couple of seconds).
	await expect
		.poll(async () => (await api(page, 'GET', '/dev/outbox')).messages.map((m: { subject: string }) => m.subject).join('\n'))
		.toContain('"Broken job" failed');
});
