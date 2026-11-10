export default function validate() {
  const changed = B.changedTasks();
  return B.acted([
    [changed.length === 1 && ['Call Bob about the roof', 'Call mum'].includes(changed[0]) && B.task(changed[0]).status === 'completed', `one call task done and nothing else (changed: ${changed.join(', ') || 'nothing'})`],
  ]);
}
