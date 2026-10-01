import { expect, test } from '@playwright/test';
import { api, newComputer, signUp, unique } from './helpers';

test('admins edit the plan catalog, give an account a credit, and see the measurements', async ({ page, browser }) => {
	// Someone to credit.
	const other = await browser.newContext({ baseURL: 'http://localhost:15173' });
	const otherPage = await other.newPage();
	const otherEmail = await signUp(otherPage, { plan: 'free', email: unique('customer') });
	await newComputer(otherPage);

	await signUp(page, { plan: 'free', email: 'admin@croncave.local' });
	await page.goto('/admin');
	await expect(page.getByRole('heading', { name: 'Admin' })).toBeVisible();

	// The catalog: change Max's allowance, save a new version.
	await page.locator('[data-field="max.allowance"]').fill('25');
	await page.getByLabel('Change note').fill('Max allowance to $25');
	await page.getByRole('button', { name: 'Save new version' }).click();
	await expect(page.getByText(/Saved catalog version \d+/)).toBeVisible();
	await expect(page.getByText('Max allowance to $25')).toBeVisible();
	const plans = await api(page, 'GET', '/plans');
	expect(plans.plans.find((p: { id: string }) => p.id === 'max').plan.allowance).toBe(15);

	// A credit for the customer, with a reason they see.
	await page.getByRole('tab', { name: 'Accounts' }).click();
	await page.getByRole('row').filter({ hasText: otherEmail }).getByRole('button', { name: 'Give credit' }).click();
	await page.getByLabel('Amount ($)').fill('7');
	await page.getByLabel('Reason (they see it)').fill('Thanks for testing the alpha');
	await page.getByRole('button', { name: 'Give credit', exact: true }).last().click();
	await expect(page.getByText('Credit given')).toBeVisible();
	await otherPage.goto('/plans');
	await expect(otherPage.getByTestId('balances')).toContainText('Credits');
	await expect(otherPage.getByTestId('balances')).toContainText('$7.00');
	await other.close();

	// Measurements from day one.
	await page.getByRole('tab', { name: 'Measurements' }).click();
	await expect(page.getByRole('heading', { name: 'Wake time' })).toBeVisible();
	await expect(page.getByText(/wakes under 5 seconds/)).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Why computers were awake' })).toBeVisible();
	await expect(page.getByText('signed up')).toBeVisible();
});
