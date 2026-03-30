import { Router, type IRouter, type Request, type Response, type NextFunction } from 'express';
import { z } from 'zod';
import {
  getDb,
  createPolicy,
  listPolicies,
  getPolicy,
  updatePolicy,
  deletePolicy,
  BUILT_IN_TEMPLATES,
} from '@phalus/core';
import type { PolicyRules } from '@phalus/core';

export const policiesRouter: IRouter = Router();

const PolicyRulesSchema = z.object({
  allow: z.array(z.string()).optional(),
  deny: z.array(z.string()).optional(),
  denyCategories: z.array(z.string()).optional(),
  allowEcosystems: z.array(z.string()).optional(),
});

const CreatePolicySchema = z.object({
  name: z.string({ required_error: '`name` is required' }).min(1, '`name` cannot be empty'),
  description: z.string().optional(),
  rules: PolicyRulesSchema,
});

const UpdatePolicySchema = z.object({
  name: z.string().min(1).optional(),
  description: z.string().nullable().optional(),
  rules: PolicyRulesSchema.optional(),
});

/**
 * Enforce Content-Type: application/json on mutation endpoints.
 */
function requireJsonContentType(req: Request, res: Response, next: NextFunction): void {
  const ct = req.headers['content-type'] ?? '';
  if (!ct.includes('application/json')) {
    res.status(415).json({ error: 'Content-Type must be application/json' });
    return;
  }
  next();
}

/**
 * GET /policies/templates
 * Returns the built-in policy templates (must be registered before /:id).
 */
policiesRouter.get('/templates', (_req, res) => {
  res.json({ templates: Object.values(BUILT_IN_TEMPLATES) });
});

/**
 * POST /policies
 * Body: { name, description?, rules } (JSON or YAML converted to JSON)
 */
policiesRouter.post('/', requireJsonContentType, (req, res) => {
  const parsed = CreatePolicySchema.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.errors[0]?.message ?? 'Invalid request' });
    return;
  }

  const { name, description, rules } = parsed.data;
  const db = getDb();
  try {
    const policy = createPolicy(db, { name, description: description ?? null, rules: rules as PolicyRules });
    res.status(201).json(policy);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (msg.includes('UNIQUE constraint')) {
      res.status(409).json({ error: `A policy named "${name}" already exists` });
    } else {
      res.status(500).json({ error: 'Internal server error' });
    }
  }
});

/**
 * GET /policies
 * Lists all user-defined policies.
 */
policiesRouter.get('/', (_req, res) => {
  const db = getDb();
  res.json({ policies: listPolicies(db) });
});

/**
 * GET /policies/:id
 * Returns a single policy by id or name.
 */
policiesRouter.get('/:id', (req, res) => {
  const db = getDb();
  const policy = getPolicy(db, req.params['id']!);
  if (!policy) {
    res.status(404).json({ error: 'Policy not found' });
    return;
  }
  res.json(policy);
});

/**
 * PATCH /policies/:id
 * Partially updates a policy (by id only).
 */
policiesRouter.patch('/:id', requireJsonContentType, (req, res) => {
  const parsed = UpdatePolicySchema.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.errors[0]?.message ?? 'Invalid request' });
    return;
  }

  const { name, description, rules } = parsed.data;
  const db = getDb();
  const updated = updatePolicy(db, req.params['id']!, { name, description, rules: rules as PolicyRules | undefined });
  if (!updated) {
    res.status(404).json({ error: 'Policy not found' });
    return;
  }
  res.json(updated);
});

/**
 * DELETE /policies/:id
 */
policiesRouter.delete('/:id', (req, res) => {
  const db = getDb();
  const deleted = deletePolicy(db, req.params['id']!);
  if (!deleted) {
    res.status(404).json({ error: 'Policy not found' });
    return;
  }
  res.status(204).end();
});
