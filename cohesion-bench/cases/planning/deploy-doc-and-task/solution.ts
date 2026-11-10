export default function solution() {
  B.ws('Acme').createDocument('Deploy process', '## Build\n\n## Release\n\n## Rollback\n');
  B.task('Document the deploy process', 'Acme').priority = 'high';
}
