import { expect, test } from '@playwright/test';
import path from 'node:path';
import { api, newComputer, signUp } from './helpers';

const fixture = (f: string) => path.join(import.meta.dirname, '..', 'fixtures', f);

test('upload, browse, preview a table, an image and text, delete to Trash and restore', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	await page.goto('/files');
	await expect(page.getByRole('heading', { name: "Your computer's files live here" })).toBeVisible();
	await page.getByRole('button', { name: 'New', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Folder' }).click();
	await page.getByLabel('Folder name').fill('Docs');
	await page.getByRole('button', { name: 'Create' }).click();
	await page.getByTestId('file-row').filter({ hasText: 'Docs' }).click();
	await expect(page).toHaveURL(/path=Docs/);

	await page.getByTestId('upload-input').setInputFiles([fixture('people.csv'), fixture('pattern.png'), fixture('notes.txt')]);
	await expect(page.getByTestId('file-row')).toHaveCount(3);
	await expect(page.getByTestId('file-row').filter({ hasText: 'people.csv' })).toContainText('Uploaded by you');

	await page.getByTestId('file-row').filter({ hasText: 'people.csv' }).click();
	await expect(page.getByTestId('preview-table')).toContainText('Grace');
	await expect(page.getByText('Table · 3 rows · 3 columns')).toBeVisible();
	await page.getByTestId('file-row').filter({ hasText: 'pattern.png' }).click();
	await expect(page.getByTestId('preview-image')).toBeVisible();
	await expect(page.getByText('64 × 48').first()).toBeVisible();
	await page.getByTestId('file-row').filter({ hasText: 'notes.txt' }).click();
	await expect(page.getByTestId('preview-text')).toContainText('water the plants');

	// Right-click: Move to Trash, then restore it from Trash.
	await page.getByTestId('file-row').filter({ hasText: 'notes.txt' }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: /Move to Trash/ }).click();
	await expect(page.getByTestId('file-row')).toHaveCount(2);
	await page.getByRole('navigation', { name: 'Places' }).getByRole('link', { name: 'Trash' }).click();
	const row = page.getByTestId('trash-row').filter({ hasText: 'notes.txt' });
	await expect(row).toContainText('You');
	await expect(row).toContainText('30');
	await row.getByRole('button', { name: 'Restore' }).click();
	await expect(page.getByTestId('trash-row')).toHaveCount(0);
	await page.goto('/files?path=Docs');
	await expect(page.getByTestId('file-row')).toHaveCount(3);
});

test('pin, rename and move, download a folder as a zip, and copy it to another computer', async ({ page }) => {
	await signUp(page, { plan: 'pro' });
	await newComputer(page, 'First box');
	const first = (await api(page, 'GET', '/me')).computers[0].id;
	await page.goto('/files');
	await page.getByRole('button', { name: 'New folder' }).click();
	await page.getByLabel('Folder name').fill('Docs');
	await page.getByRole('button', { name: 'Create' }).click();
	await page.getByTestId('file-row').filter({ hasText: 'Docs' }).click();
	await page.getByTestId('upload-input').setInputFiles([fixture('people.csv')]);
	await expect(page.getByTestId('file-row')).toHaveCount(1);

	// Rename with the menu, then move it up a level.
	await page.getByTestId('file-row').click({ button: 'right' });
	await page.getByRole('menuitem', { name: /Rename/ }).click();
	await page.getByLabel('New name').fill('team.csv');
	await page.getByRole('button', { name: 'Rename' }).click();
	await expect(page.getByTestId('file-row')).toContainText('team.csv');
	await page.getByTestId('file-row').click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Move to…' }).click();
	await page.getByRole('button', { name: 'First box', exact: true }).click();
	await page.getByRole('button', { name: 'Move here' }).click();
	await expect(page.getByTestId('file-row')).toHaveCount(0);
	await page.goto('/files');
	await expect(page.getByTestId('file-row').filter({ hasText: 'team.csv' })).toBeVisible();

	// Pin the folder; it shows under Pinned.
	await page.getByTestId('file-row').filter({ hasText: 'Docs' }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Pin to sidebar' }).click();
	await expect(page.getByRole('navigation', { name: 'Places' }).getByRole('link', { name: 'Docs' })).toBeVisible();

	// A folder downloads as a zip.
	await page.getByTestId('upload-input').setInputFiles([fixture('notes.txt')]);
	await page.getByTestId('file-row').filter({ hasText: 'notes.txt' }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Move to…' }).click();
	await page.getByRole('button', { name: 'Docs', exact: true }).click();
	await page.getByRole('button', { name: 'Move here' }).click();
	await page.getByTestId('file-row').filter({ hasText: 'Docs' }).click({ button: 'right' });
	const download = page.waitForEvent('download');
	await page.getByRole('menuitem', { name: 'Download as zip' }).click();
	expect((await download).suggestedFilename()).toBe('Docs.zip');

	// Copy the folder to a second computer.
	await newComputer(page, 'Second box');
	const second = (await api(page, 'GET', '/me')).computers.find((c: { name: string }) => c.name === 'Second box').id;
	await page.getByTestId('computer-switcher').click();
	await page.getByRole('menuitem', { name: /First box/ }).click();
	await page.goto('/files');
	await page.getByTestId('file-row').filter({ hasText: 'Docs' }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Copy to another computer' }).click();
	await page.getByRole('button', { name: 'Copy', exact: true }).click();
	await expect(page.getByText('Copied Docs to Second box')).toBeVisible({ timeout: 60_000 });
	const copied = await api(page, 'GET', `/computers/${second}/files?path=Docs`);
	expect(copied.entries.map((e: { name: string }) => e.name)).toEqual(['notes.txt']);
	expect(first).not.toBe(second);
});
