import { describe, expect, it } from 'vitest';
import { mergeFiles } from '../../src/composables/files';

describe('mergeFiles', () => {
	it('appends new files after the ones already selected, without duplicates', () => {
		expect(mergeFiles(['/a.jpg', '/b.txt'], ['/b.txt', '/c.png'])).toEqual(['/a.jpg', '/b.txt', '/c.png']);
	});

	it('keeps the order of the incoming files and drops duplicates inside them', () => {
		expect(mergeFiles([], ['/x', '/y', '/x'])).toEqual(['/x', '/y']);
	});

	it('is the incoming list when nothing was selected', () => {
		expect(mergeFiles(undefined, ['/only'])).toEqual(['/only']);
	});
});
