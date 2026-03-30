import express from 'express';
import { SCHEMA_VERSION } from '@phalus/core';

const app = express();
const PORT = process.env.PORT ? parseInt(process.env.PORT) : 3000;

app.use(express.json());

// Health endpoint
app.get('/health', (_req, res) => {
  res.json({
    status: 'ok',
    version: process.env.npm_package_version ?? '0.1.0',
    schemaVersion: SCHEMA_VERSION,
    uptime: process.uptime(),
  });
});

// OpenAPI spec stub — will be fleshed out in Phase 1
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
        get: {
          summary: 'Health check',
          responses: {
            '200': { description: 'Service is healthy' },
          },
        },
      },
    },
  });
});

app.listen(PORT, () => {
  console.log(`PHALUS API listening on port ${PORT}`);
});

export default app;
