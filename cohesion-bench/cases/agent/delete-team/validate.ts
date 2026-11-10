export default function validate() {
  const titles = cohesion.teams().map((team) => team.title);
  return B.check([
    [!titles.includes('Old projects'), 'Old projects gone'],
    [titles.includes('Writing'), 'Writing kept'],
  ]);
}
