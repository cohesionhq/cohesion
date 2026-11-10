export default function validate() {
  const titles = cohesion.teams().map((team) => team.title);
  return B.check([[titles.includes('Research') && !titles.includes('Reserch'), `renamed (teams: ${titles.join(', ')})`]]);
}
