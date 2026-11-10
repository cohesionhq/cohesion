export default function setup() {
  const old = B.ws().createDocumentFolder('Old stuff');
  old.createDocument('Old router manual', 'Hold reset for 10 seconds.');
  old.createDocument('2019 tax notes', 'Filed in January 2020.');
}
