import express, { type Request, type Response, type NextFunction } from 'express';
import { SCHEMA_VERSION, initDb } from '@phalus/core';
import { requireApiKey } from './middleware/auth.js';
import { defaultRateLimit } from './middleware/rate-limit.js';
import { scansRouter } from './routes/scans.js';
import { licensesRouter } from './routes/licenses.js';
import { policiesRouter } from './routes/policies.js';

// Initialize DB (uses PHALUS_DB_PATH or ./phalus.db)
initDb();

const app: express.Application = express();
app.use(express.json());
app.use(requireApiKey);
app.use(defaultRateLimit);

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
            content: {
              'application/json': {
                schema: {
                  type: 'object',
                  properties: {
                    path: { type: 'string', description: 'Project path to scan' },
                    policyId: { type: 'string', description: 'Optional policy id or name to evaluate against scan results' },
                  },
                  required: ['path'],
                },
              },
            },
          },
          responses: { '200': { description: 'Scan result with optional policyVerdict and policyViolations' }, '400': { description: 'Bad request' } },
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
      '/policies/templates': {
        get: { summary: 'List built-in policy templates', responses: { '200': { description: 'Built-in templates' } } },
      },
      '/policies': {
        post: {
          summary: 'Create a new policy',
          requestBody: {
            required: true,
            content: {
              'application/json': {
                schema: {
                  type: 'object',
                  properties: {
                    name: { type: 'string' },
                    description: { type: 'string' },
                    rules: {
                      type: 'object',
                      properties: {
                        allow: { type: 'array', items: { type: 'string' } },
                        deny: { type: 'array', items: { type: 'string' } },
                        denyCategories: { type: 'array', items: { type: 'string' } },
                        allowEcosystems: { type: 'array', items: { type: 'string' } },
                      },
                    },
                  },
                  required: ['name', 'rules'],
                },
              },
            },
          },
          responses: { '201': { description: 'Policy created' }, '400': { description: 'Bad request' }, '409': { description: 'Name conflict' } },
        },
        get: { summary: 'List all policies', responses: { '200': { description: 'Policy list' } } },
      },
      '/policies/{id}': {
        get: { summary: 'Get a policy by id or name', responses: { '200': { description: 'Policy object' }, '404': { description: 'Not found' } } },
        patch: { summary: 'Update a policy', responses: { '200': { description: 'Updated policy' }, '404': { description: 'Not found' } } },
        delete: { summary: 'Delete a policy', responses: { '204': { description: 'Deleted' }, '404': { description: 'Not found' } } },
      },
    },
  });
});

app.use('/scans', scansRouter);
app.use('/licenses', licensesRouter);
app.use('/policies', policiesRouter);

// Global error handler — must be last. Never leak stack traces or internal details.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
app.use((err: Error, _req: Request, res: Response, _next: NextFunction) => {
  res.status(500).json({ error: 'Internal server error' });
});

export default app;
