export default function validate() {
  const want = B.taskFolder('Imported').tasks({}).filter((t) => /invoice/i.test(t.title)).length;
  return B.answered([[B.saysNumber(reply, want), `said ${want} (said: ${reply.slice(0, 200)})`]])
}
