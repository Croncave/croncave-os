import { expect, test } from '@playwright/test';
import { newComputer, signUp } from './helpers';

test('add a script, test it, save it, run it with live output, and see its result and files', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	await page.goto('/scripts');
	await page.getByRole('link', { name: 'Add a script' }).click();
	await page.getByRole('button', { name: 'Summarize a spreadsheet (Python)' }).click();
	await expect(page.getByRole('button', { name: /Script report\.py/ })).toBeVisible();
	await expect(page.getByTestId('plain-words')).toContainText('Run report.py only when you press Run. Only tell me if it fails.');
	await page.getByRole('button', { name: /Script report\.py/ }).click();
	await page.getByLabel('Script name').fill('Sales summary');

	// A test run before saving shows what it did.
	await page.getByRole('button', { name: 'Test it' }).click();
	await expect(page.getByTestId('test-run')).toContainText('10 sales summarized; East sold the most.');
	await expect(page.getByTestId('test-run')).toContainText('summary.csv');
	await page.getByRole('button', { name: 'Save script' }).click();
	await expect(page).toHaveURL(/\/scripts\/[0-9a-f-]+$/);
	await expect(page.getByTestId('when-it-runs')).toHaveText('Run only when you press Run, and tell me if it fails.');

	await page.getByRole('button', { name: 'Run now' }).click();
	await expect(page).toHaveURL(/tab=runs&run=/);
	await expect(page.getByTestId('output')).toContainText('East: $81.24');
	await expect(page.getByTestId('headline')).toHaveText('10 sales summarized; East sold the most');
	await expect(page.getByText('Top region')).toBeVisible();
	await page.getByRole('link', { name: 'Files › Scripts › csv-report › summary.csv' }).click();
	await expect(page.getByTestId('file-row').filter({ hasText: 'summary.csv' })).toContainText('Sales summary · run at');

	// A schedule faster than the Free plan allows is refused in plain words.
	await page.goto('/scripts');
	await page.getByRole('link', { name: /Sales summary/ }).click();
	await page.getByRole('navigation', { name: 'Script' }).getByRole('link', { name: 'Settings' }).click();
	await page.getByRole('button', { name: /When to run/ }).click();
	await page.getByRole('radio', { name: 'On a schedule' }).click();
	await page.getByRole('button', { name: 'Write a custom schedule' }).click();
	await page.getByLabel('Cron schedule').fill('*/5 * * * *');
	await page.getByRole('button', { name: 'Save changes' }).click();
	await expect(page.getByText(/runs jobs at most every hour/)).toBeVisible();
	await page.getByLabel('Cron schedule').fill('0 9 * * *');
	await page.getByRole('checkbox', { name: 'Only between' }).check();
	await page.getByLabel('Start time').fill('08:00');
	await page.getByLabel('End time').fill('18:00');
	await page.getByRole('button', { name: 'Save changes' }).click();
	await expect(page.getByText('Saved', { exact: true })).toBeVisible();
	await page.getByRole('navigation', { name: 'Script' }).getByRole('link', { name: 'Overview' }).click();
	await expect(page.getByTestId('when-it-runs')).toHaveText('Run every day at 9:00 am ET from 8 AM to 6 PM, and tell me if it fails.');
});

test('a failing script says why in plain words, with the fix', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	const me = await (await page.request.get('/api/me')).json();
	await page.request.put(`/api/computers/${me.computers[0].id}/files/write?path=Scripts/fetch.py`, { data: 'import requests\nprint(requests.get("x"))\n' });
	const made = await (await page.request.post(`/api/computers/${me.computers[0].id}/scripts`, { data: { name: 'fetch.py', path: 'Scripts/fetch.py', run_now: true } })).json();
	await page.goto(`/runs/${made.run_id}`);
	await expect(page).toHaveURL(/tab=runs/);
	await expect(page.getByTestId('why')).toContainText('needs the Python package "requests"');
	await expect(page.getByTestId('why')).toContainText('Add requests to a requirements.txt file');
});
