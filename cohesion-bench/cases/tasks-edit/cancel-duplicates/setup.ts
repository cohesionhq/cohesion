export default function setup() {
  B.ws().createTask({ title: 'Buy printer ink', folder: B.taskFolder('Errands') });
  B.ws().createTask({ title: 'Call mum' });
  B.ws().createTask({ title: 'Read Piranesi' });
}
