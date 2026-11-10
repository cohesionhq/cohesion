export default function validate() {
  const tasks = B.taskFolder('Imported').tasks({ status: B.ALL });
  const temp = tasks.filter((t) => t.title.startsWith('TEMP'));
  return B.check([
    [temp.every((t) => t.status === 'cancelled'), `every TEMP task cancelled (${temp.filter((t) => t.status === 'cancelled').length} of ${temp.length})`],
    [tasks.filter((t) => !t.title.startsWith('TEMP')).every((t) => t.status === 'incomplete'), 'the others left open'],
  ])
}
