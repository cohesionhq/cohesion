export default function validate() {
  const vehicles = B.findTaskFolder('Vehicles');
  return B.check([
    [vehicles !== null, 'a Vehicles folder'],
    [vehicles && B.task('Fix the bike puncture').taskFolderId === vehicles.id, 'the bike task moved'],
    [vehicles && B.task('Book the car service').taskFolderId === vehicles.id, 'the car task moved'],
    [B.changedOnly(['Fix the bike puncture', 'Book the car service']), `nothing else moved (changed: ${B.changedTasks().join(', ')})`],
  ])
}
