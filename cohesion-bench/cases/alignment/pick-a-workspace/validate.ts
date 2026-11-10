export default function validate() {
  return B.acted([[B.tasksLike(/venue/i, B.ANY).length === 1, 'one venue task, in either workspace']]);
}
