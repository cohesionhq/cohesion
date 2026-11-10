export default function validate() {
  const want = B.taskFolder('Imported').tasks({}).reduce((sum, t) => sum + (t.size ?? 0), 0);
  return B.answered([[B.saysNumber(reply, want), `said ${want} (said: ${reply.slice(0, 200)})`]]);
}
