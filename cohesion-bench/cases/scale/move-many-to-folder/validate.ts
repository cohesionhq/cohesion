export default function validate() {
  const kitchen = B.findTaskFolder('Kitchen');
  const all = B.tasks();
  const want = all.filter((t) => t.title.startsWith('Kitchen:'));
  const misplaced = all.filter((t) => (t.taskFolderId === kitchen?.id) !== t.title.startsWith('Kitchen:'));
  return B.check([
    [kitchen !== null, 'a Kitchen folder'],
    [want.length === 40 && misplaced.length === 0, `the 40 kitchen tasks in Kitchen and nothing else (${misplaced.length} misplaced)`],
  ]);
}
