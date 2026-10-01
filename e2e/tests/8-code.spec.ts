import { expect, test } from '@playwright/test';
import { newComputer, signUp } from './helpers';

test('open a project, a mock agent makes changes you approve and review, a dev server runs in a private preview', async ({ page }) => {
	test.setTimeout(150_000);
	await signUp(page);
	await newComputer(page);
	await page.goto('/code');
	await page.getByLabel('New project name').fill('bakery');
	await page.getByRole('button', { name: 'New project' }).click();
	await expect(page.getByTestId('project')).toHaveValue('Projects/bakery');

	// Edit a file in the editor and save it.
	await page.getByRole('navigation', { name: 'Project files' }).getByRole('button', { name: 'README.md' }).click();
	await expect(page.getByTestId('editor')).toContainText('A tiny static site');
	await page.getByTestId('editor').locator('.cm-content').click();
	await page.keyboard.press('Control+End');
	await page.keyboard.type('\nEdited by hand.');
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByText('Saved')).toBeVisible();

	// Hand the agent a task. It asks before running a command.
	await page.getByLabel('Task for the agent').fill('Make the heading say "Fresh bread daily"');
	await page.getByRole('button', { name: 'Hand it to the agent' }).click();
	await expect(page.getByTestId('approval')).toContainText('ls -la');
	await expect(page.getByTestId('computer-switcher')).toContainText('Needs you');
	await page.getByTestId('approval').getByRole('button', { name: 'Approve' }).click();
	await expect(page.getByTestId('output')).toContainText('Approved. Running: ls -la');
	await page.getByRole('button', { name: 'Review changes' }).click();

	// Review file by file: keep the page, undo the changelog.
	const files = page.getByTestId('review-file');
	await expect(files).toHaveCount(3);
	const index = files.filter({ hasText: 'index.html' });
	await expect(index).toContainText('<h1>Fresh bread daily</h1>');
	await index.getByRole('button', { name: 'Keep' }).click();
	await files.filter({ hasText: 'CHANGELOG.md' }).getByRole('button', { name: 'Undo' }).click();
	await expect(files.filter({ hasText: 'CHANGELOG.md' })).toContainText('Undone');
	await expect(page.getByRole('navigation', { name: 'Project files' })).not.toContainText('CHANGELOG.md');

	// Run the dev server and open it in a private preview.
	await page.getByRole('tab', { name: /Preview/ }).click();
	await page.getByRole('button', { name: 'Start dev server' }).click();
	await expect(page.getByRole('button', { name: 'Open preview' })).toBeVisible({ timeout: 30_000 });
	await page.getByRole('button', { name: 'Open preview' }).click();
	const frame = page.frameLocator('[data-testid=preview-frame]');
	await expect(frame.getByRole('heading', { name: 'Fresh bread daily' })).toBeVisible();
	await page.getByRole('button', { name: 'Phone' }).click();
	await expect(page.getByTestId('preview-frame')).toHaveClass(/phone/);

	// Without the cookie the preview is private.
	const origin = new URL((await page.getByTestId('preview-frame').getAttribute('src'))!).origin;
	const anon = await page.context().browser()!.newContext();
	const stranger = await anon.newPage();
	await stranger.goto(origin);
	await expect(stranger.getByRole('heading', { name: 'This preview is private' })).toBeVisible();
	await anon.close();
});
