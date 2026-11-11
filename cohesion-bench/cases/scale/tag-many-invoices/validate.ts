export default function validate() {
  const tasks = B.taskFolder('Imported').tasks({ status: B.ALL });
  const invoices = tasks.filter((t) => /invoice/i.test(t.title));
  const wrong = tasks.filter((t) => B.tagged(t, 'billing') !== /invoice/i.test(t.title));
  return B.check([
    [invoices.every((t) => B.tagged(t, 'billing')), `every invoice task tagged (${invoices.filter((t) => B.tagged(t, 'billing')).length} of ${invoices.length})`],
    [wrong.length === 0, `no other task tagged (${wrong.length} wrong)`],
  ]);
}
