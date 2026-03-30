import app from './index.js';

const PORT = process.env['PORT'] ? parseInt(process.env['PORT']) : 3000;
app.listen(PORT, () => {
  console.log(`PHALUS API listening on port ${PORT}`);
});
