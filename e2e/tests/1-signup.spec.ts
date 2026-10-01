import { expect, test } from '@playwright/test';
import { phone, signInLink, smsCode, unique } from './helpers';

test('sign up with an email link and a phone code, pay for Plus, get the award, start the trial', async ({ page }) => {
	const email = unique('signup');
	const number = phone();
	await page.goto('/');
	await expect(page).toHaveURL(/signin/);
	await page.getByLabel('Email').fill(email);
	await page.getByRole('button', { name: 'Email me a sign-in link' }).click();
	await expect(page.getByText('Check your email')).toBeVisible();

	// The sign-in link arrives in the outbox (Dev tools).
	await page.getByTestId('outbox-link').click();
	await expect(page.getByTestId('outbox')).toContainText('Your Croncave sign-in link');
	await page.goto(await signInLink(page.request, email));

	await expect(page.getByRole('heading', { name: 'Verify your phone' })).toBeVisible();
	await page.getByLabel('Mobile number').fill('+44 20 7946 0958');
	await page.getByRole('button', { name: 'Text me a code' }).click();
	await expect(page.getByText(/only available in the United States/)).toBeVisible();
	await page.getByLabel('Mobile number').fill(number);
	await page.getByRole('button', { name: 'Text me a code' }).click();
	await page.getByLabel('Code').fill('000000');
	await page.getByRole('button', { name: 'Verify' }).click();
	await expect(page.getByText(/code isn't right/)).toBeVisible();
	await page.getByLabel('Code').fill(await smsCode(page.request, number));
	await page.getByRole('button', { name: 'Verify' }).click();

	await expect(page.getByRole('heading', { name: 'Choose a plan' })).toBeVisible();
	await expect(page.locator('[data-plan=plus]')).toContainText('$10');
	await page.locator('[data-plan=plus]').getByRole('button').click();
	// A declined test card, then a good one.
	await page.getByLabel('Card number').fill('4000 0000 0000 0002');
	await page.getByLabel('Security code').fill('123');
	await page.getByRole('button', { name: /^Pay \$10/ }).click();
	await expect(page.getByTestId('card-error')).toHaveText('Your card was declined.');
	await page.getByLabel('Card number').fill('4242 4242 4242 4242');
	await page.getByRole('button', { name: /^Pay \$10/ }).click();

	await expect(page.getByTestId('award')).toContainText('$5.00');
	await expect(page.getByRole('heading', { name: 'Try Pro free for 7 days' })).toBeVisible();
	await expect(page.getByText('Nothing is charged automatically.')).toBeVisible();
	await page.getByRole('button', { name: 'Start the free trial' }).click();
	await expect(page).toHaveURL(/computers\/new/);

	await page.goto('/plans');
	await expect(page.getByTestId('trial-banner')).toContainText("You're trying Pro: 7 days left.");
	await expect(page.getByTestId('balances')).toContainText('Sign-up award');
});
