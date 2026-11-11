export default function solution() {
  return String(B.taskFolder('Imported').tasks({}).reduce((sum, t) => sum + (t.size ?? 0), 0));
}
