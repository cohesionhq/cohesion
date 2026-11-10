export default function solution() {
  return ({ cues: [{ name: 'Urgent tasks', hook: 'task:create', code: '(task) => task.priority === TaskPriority.urgent', instructions: 'Tell the user an urgent task was added.', effort: 'low' }] })
}
