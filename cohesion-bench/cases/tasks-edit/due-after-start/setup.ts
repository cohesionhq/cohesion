export default function setup() {
  const launch = B.ws().createTaskFolder({ title: 'Launch' });
  launch.create({ title: 'Draft the press release', startDate: '2026-12-01' });
  launch.create({ title: 'Brief the support team', startDate: '2026-12-03' });
  launch.create({ title: 'Send the launch email', startDate: '2026-12-10' });
}
