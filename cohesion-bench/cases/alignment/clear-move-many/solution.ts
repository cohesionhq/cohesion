export default function solution() {
  for (const t of B.taskFolder('Work').tasks({ status: B.ALL })) t.moveTo(B.taskFolder('Home'));
}
