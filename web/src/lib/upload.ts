import { api, post, get } from './api';

const CHUNK = 1024 * 1024;

/**
 * Upload a file in chunks. A dropped connection resumes from what the computer really
 * has, so a large upload survives a flaky network.
 */
export async function uploadFile(computer: string, file: File, path: string, progress: (sent: number) => void): Promise<void> {
	const { upload_id } = await post(`/computers/${computer}/uploads`, { path, size: file.size });
	let offset = 0;
	let failures = 0;
	while (offset < file.size) {
		const chunk = file.slice(offset, Math.min(file.size, offset + CHUNK));
		try {
			const r = await api(`/computers/${computer}/uploads/${upload_id}?offset=${offset}`, { method: 'PUT', raw: chunk });
			offset = r.received;
			failures = 0;
			progress(offset);
		} catch (e) {
			if (++failures > 6) throw e;
			await new Promise((res) => setTimeout(res, 500 * 2 ** failures));
			try {
				offset = (await get(`/computers/${computer}/uploads/${upload_id}`)).received;
			} catch {
				// Still offline; try again.
			}
		}
	}
	await post(`/computers/${computer}/uploads/${upload_id}/finish`, { path });
	progress(file.size);
}
