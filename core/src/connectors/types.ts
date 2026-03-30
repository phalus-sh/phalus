import type { Ecosystem } from '../types.js';

export interface ScannedPackage {
  ecosystem: Ecosystem;
  name: string;
  version: string;
  licenseExpression?: string;
  licenseSource?: string;
}
