export default function validate() {
  const titles = cohesion.workspaces().map((w) => w.title);
  return B.check([[titles.includes('Acme Corp') && !titles.includes('Acme'), `renamed (workspaces: ${titles.join(', ')})`]])
}
