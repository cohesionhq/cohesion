export default function validate() {
  const titles = B.newTasks(B.ANY).map((t) => t.title);
  return B.check([
    [titles.some((t) => /bank|mortgage/i.test(t)), 'a task to ring the bank'],
    [titles.some((t) => /card/i.test(t) && /sam/i.test(t)), "a task for Sam's card"],
    [titles.length === 2, `exactly two new tasks (found ${titles.length})`],
  ])
}
