// Core domain types for PHALUS

export type Ecosystem = 'npm' | 'pip' | 'cargo' | 'go' | 'maven' | 'nuget' | 'ruby' | 'php' | 'unknown';

export interface PolicyRules {
  /** Explicit license allow list (SPDX IDs). Allow overrides deny. */
  allow?: string[];
  /** Explicit license deny list (SPDX IDs). */
  deny?: string[];
  /** Deny licenses by category. */
  denyCategories?: LicenseCategory[];
  /** Restrict scanning to these ecosystems only (informational). */
  allowEcosystems?: Ecosystem[];
}

export interface Policy {
  id: string;
  name: string;
  description: string | null;
  rules: PolicyRules;
  createdAt: string;
  updatedAt: string;
}

export interface PolicyViolation {
  packageName: string;
  packageVersion: string;
  ecosystem: string;
  license: string | null;
  rule: string;
  remediationHint: string;
}

export interface PolicyResult {
  verdict: 'pass' | 'fail';
  violations: PolicyViolation[];
}

export type LicenseCategory =
  | 'permissive'
  | 'copyleft-weak'
  | 'copyleft-strong'
  | 'proprietary'
  | 'unknown';

export type ScanStatus = 'pending' | 'running' | 'done' | 'failed';
export type AlertSeverity = 'low' | 'medium' | 'high' | 'critical';
export type AlertKind = 'proprietary-license' | 'strong-copyleft' | 'license-missing' | 'policy-violation';

export interface Package {
  id: string;
  ecosystem: Ecosystem;
  name: string;
  version: string;
  licenseExpression: string | null;
  licenseSource: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ScanRun {
  id: string;
  projectPath: string;
  status: ScanStatus;
  startedAt: string | null;
  finishedAt: string | null;
  error: string | null;
  createdAt: string;
}

export interface Alert {
  id: string;
  packageId: string | null;
  scanRunId: string | null;
  kind: AlertKind;
  severity: AlertSeverity;
  message: string;
  resolvedAt: string | null;
  createdAt: string;
}
