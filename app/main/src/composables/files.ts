/**
 * Files already selected plus the ones that just arrived (drag and drop,
 * argv or the single-instance "send_files" event), in that order and
 * without duplicates.
 */
export function mergeFiles(current: string[] | undefined, incoming: string[]): string[] {
	const merged = [...(current ?? [])];
	for (const file of incoming) {
		if (!merged.includes(file)) merged.push(file);
	}
	return merged;
}
