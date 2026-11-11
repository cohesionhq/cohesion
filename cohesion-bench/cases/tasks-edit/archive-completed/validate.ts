export default function validate() {
  const done = B.findTaskFolder('Done');
  const completed = ['Replace the hallway light bulb', 'Post the parcel to Grandma', 'Set up the ISA transfer'];
  return B.check([
    [done !== null, 'a Done folder'],
    ...completed.map((title) => [done && B.task(title).taskFolderId === done.id, `${title} in Done`]),
    [B.changedOnly(completed), `moved only the completed tasks (changed: ${B.changedTasks().join(', ')})`],
  ])
}
