export default function validate() {
  return B.check([[cohesion.teams().some((team) => team.title === 'Research'), 'a Research team']])
}
