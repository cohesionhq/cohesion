export default function validate() {
  const reports = B.tasksLike(/^Weekly report/);
  const wrong = reports.filter((t) => {
    const week = Number(t.title.match(/-?\d+/)![0]);
    const original = B.day(-week);
    return t.dueDate !== (week > 0 ? B.day(-week + 7) : original);
  });
  return B.check([[wrong.length === 0, `every overdue report moved a week, the rest left (${wrong.length} wrong: ${wrong.slice(0, 3).map((t) => `${t.title} ${t.dueDate}`).join(', ')})`]])
}
