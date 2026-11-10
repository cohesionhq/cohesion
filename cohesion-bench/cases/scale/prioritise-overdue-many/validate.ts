export default function validate() {
  const tasks = B.taskFolder('Imported').tasks({ status: B.ALL });
  const overdue = (t: Task) => t.status === 'incomplete' && t.dueDate !== null && t.dueDate < B.day(0);
  const wrong = tasks.filter((t) => (t.priority === 'high') !== overdue(t));
  return B.check([[wrong.length === 0, `the overdue open tasks high and the rest left (${wrong.length} wrong: ${wrong.slice(0, 3).map((t) => `${t.title} ${t.dueDate} ${t.status} ${t.priority}`).join(', ')})`]]);
}
