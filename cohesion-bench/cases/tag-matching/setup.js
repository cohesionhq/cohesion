const w = cohesion.workspace('Personal');
for (const title of ['Send the October invoice to Acme', 'Chase the unpaid invoice from Globex', 'Buy a birthday card', 'Update my CV', 'File the invoice receipts']) {
  w.createTask({ title });
}
