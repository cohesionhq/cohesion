export default function validate() {
  return B.check([
    [B.tasksLike(/^Call Bob about the roof flashing$/).length === 1, 'renamed'],
    [B.tasksLike(/^Call Bob about the roof$/).length === 0, 'the old title is gone'],
    [B.tasks().length === B.before().tasks.length, 'no task added'],
  ])
}
