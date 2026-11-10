export default function setup() {
  B.ws().createContact({ firstName: 'Dana', lastName: 'Wu', methods: [{ kind: 'email', value: 'dana@example.com' }] });
  B.ws().createContact({ firstName: 'Dana', lastName: 'Wu', methods: [{ kind: 'phone', value: '+44 7700 900555' }] });
}
