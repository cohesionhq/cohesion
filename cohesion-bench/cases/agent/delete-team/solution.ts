export default function solution() {
  cohesion.teams().find((team) => team.title === 'Old projects')!.delete();
}
