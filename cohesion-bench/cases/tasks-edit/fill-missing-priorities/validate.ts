export default function validate() {
  return B.check([
    [B.task('Order contact lenses').priority === 'medium', 'contact lenses medium'],
    [B.task('Get a flu jab').priority === 'medium', 'flu jab medium'],
    [B.changedOnly(['Order contact lenses', 'Get a flu jab']), `changed only Order contact lenses, Get a flu jab (changed: ${B.changedTasks().join(', ')})`],
  ])
}
