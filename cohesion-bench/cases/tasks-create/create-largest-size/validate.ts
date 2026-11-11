export default function validate() {
  const [task] = B.tasksLike(/loft/i, B.ANY);
  return B.check([[task?.size === 5, `size 5 (was ${task?.size})`]])
}
