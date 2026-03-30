import express from 'express';
import { SCHEMA_VERSION, initDb } from '@phalus/core';
import { requireApiKey } from './middleware/auth.js';
import { scansRouter } from './routes/scans.js';
import { licensesRouter } from './routes/licenses.js';

// Initialize DB (uses PHALUS_DB_PATH or ./phalus.db)
initDb();

const app: express.Application = express();
app.use(express.json());
app.use(requireApiKey);

app.get('/health', (_req, res) => {
  res.json({
    status: 'ok',
    version: process.env['npm_package_version'] ?? '0.1.0',
    schemaVersion: SCHEMA_VERSION,
    uptime: process.uptime(),
  });
});

app.get('/openapi.json', (_req, res) => {
  res.json({
    openapi: '3.1.0',
    info: {
      title: 'PHALUS API',
      version: '0.1.0',
      description: 'Private Headless Automated License Uncoupling System',
    },
    paths: {
      '/health': {
        get: { summary: 'Health check', responses: { '200': { description: 'Service is healthy' } } },
      },
      '/scans': {
        post: {
          summary: 'Trigger a new scan',
          requestBody: {
            required: true,
            content: { 'application/json': { schema: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'] } } },
          },
          responses: { '200': { description: 'Scan result' }, '400': { description: 'Bad request' } },
        },
        get: { summary: 'List all scan runs', responses: { '200': { description: 'List of scan runs' } } },
      },
      '/scans/{id}': {
        get: { summary: 'Get a scan run by ID', responses: { '200': { description: 'Scan run details' }, '404': { description: 'Not found' } } },
      },
      '/licenses': {
        get: {
          summary: 'List packages with license info',
          parameters: [
            { name: 'q', in: 'query', schema: { type: 'string' } },
            { name: 'ecosystem', in: 'query', schema: { type: 'string' } },
            { name: 'category', in: 'query', schema: { type: 'string' } },
            { name: 'limit', in: 'query', schema: { type: 'integer', default: 100 } },
            { name: 'offset', in: 'query', schema: { type: 'integer', default: 0 } },
          ],
          responses: { '200': { description: 'License list' } },
        },
      },
    },
  });
});

app.use('/scans', scansRouter);
app.use('/licenses', licensesRouter);

export default app;
