export default function validate() {
  const errands = B.taskFolder('Errands').tasks({});
  const items = ['egg', 'flour', 'butter', 'vanilla'];
  return B.check(items.map((item) => [
    errands.filter((t) => B.mentions(t.title, item)).length === 1,
    `one Errands task for ${item}`,
  ]))
}
