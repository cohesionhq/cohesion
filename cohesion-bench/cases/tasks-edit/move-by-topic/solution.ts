export default function solution() {
  const folder = B.ws().createTaskFolder({ title: 'Vehicles' });
  B.task('Fix the bike puncture').moveTo(folder);
  B.task('Book the car service').moveTo(folder);
}
