export default function solution() {
  return `There are ${B.taskFolder('Imported').tasks({}).filter((t) => /invoice/i.test(t.title)).length}.`
}
