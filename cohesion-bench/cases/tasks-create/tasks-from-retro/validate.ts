export default function validate() {
  const titles = B.tasks().map((t) => t.title);
  const covered = [/runbook/i, /alert/i, /code review/i].map((pattern) => titles.some((title) => pattern.test(title)));
  return B.check([
    [covered[0], 'a runbook task'], [covered[1], 'an alerts task'], [covered[2], 'a code review task'],
    [titles.length === 3, `exactly three tasks (found ${titles.length})`],
  ])
}
