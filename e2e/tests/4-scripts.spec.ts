import { expect, test } from '@playwright/test';
import { newComputer, signUp } from './helpers';

test('add a script, run it now with live output, see its summary and the files it made', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	await page.goto('/scripts');
	await page.getByRole('button', { name: 'Add a script' }).first().click();
	await page.getByRole('button', { name: 'Summarize a spreadsheet (Python)' }).click();
	await expect(page.getByLabel('Script path')).toHaveValue('Scripts/csv-report/report.py');
	await expect(page.getByTestId('plain-words')).toContainText('Run Scripts/csv-report/report.py when I press Run now');
	await page.getByLabel('Job name').fill('Sales summary');
	await page.getByRole('button', { name: 'Save and run now' }).click();

	await expect(page).toHaveURL(/\/runs\//);
	await expect(page.getByTestId('output')).toContainText('East: $81.24');
	await expect(page.getByTestId('headline')).toHaveText('10 sales summarized; East sold the most');
	await expect(page.getByText('Top region')).toBeVisible();
	await page.getByRole('link', { name: 'Scripts/csv-report/summary.csv' }).click();
	await expect(page.getByTestId('file-row').filter({ hasText: 'summary.csv' })).toContainText('Made by "Sales summary"');
	await expect(page.getByTestId('preview-table')).toContainText('West');

	// A schedule faster than the Free plan allows is refused in plain words.
	await page.goto('/scripts');
	await page.getByRole('link', { name: 'Sales summary' }).click();
	await page.getByRole('button', { name: 'Change' }).click();
	await page.getByRole('radio', { name: 'On a schedule' }).check();
	await page.getByRole('button', { name: 'Write a custom schedule' }).click();
	await page.getByLabel('Cron schedule').fill('*/5 * * * *');
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByText(/runs jobs at most every hour/)).toBeVisible();
	await page.getByLabel('Cron schedule').fill('0 9 * * *');
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByText('Every day at 9:00 am ET')).toBeVisible();
});

test('a failing script says why in plain words, with the fix', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	const me = await (await page.request.get('/api/me')).json();
	await page.request.put(`/api/computers/${me.computers[0].id}/files/write?path=Scripts/fetch.py`, { data: 'import requests\nprint(requests.get("x"))\n' });
	await page.goto('/scripts');
	await page.getByRole('button', { name: 'Add a script' }).first().click();
	await page.getByLabel('Job name').fill('Fetch');
	await page.getByLabel('Script path').fill('Scripts/fetch.py');
	await page.getByRole('button', { name: 'Save and run now' }).click();
	await expect(page.getByTestId('why')).toContainText('needs the Python package "requests"');
	await expect(page.getByTestId('why')).toContainText('Add requests to a requirements.txt file');
});
