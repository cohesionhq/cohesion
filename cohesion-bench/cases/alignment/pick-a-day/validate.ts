export default function validate() {
  const [task] = B.tasksLike(/plumber/i, B.ANY);
  const due = task?.dueDate ?? '';
  return B.acted([[due >= B.day(1) && due <= B.day(14), `due within the next two weeks (was ${task?.dueDate})`]]);
}
