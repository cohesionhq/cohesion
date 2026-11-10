const tasks = cohesion.workspace('Personal').tasks().filter((t) => /stamp/i.test(t.title));
const task = tasks[0];
({
  pass: tasks.length === 1 && task.dueDate === '2026-11-14' && task.priority === 'high',
  reason: tasks.length === 1
    ? `due ${task.dueDate}, priority ${task.priority}`
    : `${tasks.length} tasks about stamps`,
})
