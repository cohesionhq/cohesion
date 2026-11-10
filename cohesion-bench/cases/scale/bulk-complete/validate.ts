export default function validate() {
  const all = B.tasks();
  const tagged = all.filter((t) => B.tagged(t, 'import'));
  return B.check([
    [tagged.length === 150 && tagged.every((t) => t.status === 'completed'), `all 150 import tasks completed (${tagged.filter((t) => t.status === 'completed').length} were)`],
    [all.filter((t) => !B.tagged(t, 'import')).every((t) => t.status === 'incomplete'), 'the others left open'],
  ])
}
