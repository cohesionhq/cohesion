export default function solution() {
  B.ws().createContact({ firstName: 'Ada', lastName: 'Lovelace', methods: [{ kind: 'email', value: 'ada@example.com' }] });
}
