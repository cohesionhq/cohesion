export default function validate() {
  const old = B.findDocFolder('Old drafts');
  const drafts = B.docFolder('Drafts').documents();
  const moved = old?.documents() ?? [];
  return B.check([
    [old !== null, 'an Old drafts folder'],
    [moved.length === 50 && moved.every((d) => d.title.startsWith('Untitled')), `the 50 Untitled documents in it (found ${moved.length})`],
    [drafts.length === 30 && drafts.every((d) => !d.title.startsWith('Untitled')), `the 30 others left in Drafts (found ${drafts.length})`],
  ]);
}
