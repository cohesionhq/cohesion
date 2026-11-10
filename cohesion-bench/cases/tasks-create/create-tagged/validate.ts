export default function validate() {
  const [task] = B.tasksLike(/grandma/i).filter((t) => t.status === 'incomplete');
  return B.check([
    [task && B.tagged(task, 'family'), 'tagged family'],
    [task && B.tagged(task, 'phone'), 'tagged phone'],
    [B.ws().tags().length === 5, 'no new tags made'],
  ])
}
