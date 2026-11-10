export default function setup() {
  const plan = B.doc('Launch plan', 'Acme');
  plan.contents = `${plan.contents}\n## Dependencies\n\nFalcon must be live before the release.\n`;
}
