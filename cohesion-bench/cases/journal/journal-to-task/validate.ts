export default function validate() {
  return B.check([[B.newTasks().some((t) => /parking permit/i.test(t.title)), 'a task to renew the parking permit']])
}
