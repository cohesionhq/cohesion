export default function validate() {
  const [task] = B.tasksLike(/loft/i);
  return B.check([[task?.size === 5, `size 5 (was ${task?.size})`]])
}
