import { describe, it, expect } from 'vitest';
import { normalizeLicense, classifyLicense } from './license-data.js';

describe('normalizeLicense', () => {
  it('returns SPDX canonical for known aliases', () => {
    expect(normalizeLicense('MIT')).toBe('MIT');
    expect(normalizeLicense('mit')).toBe('MIT');
    expect(normalizeLicense('Apache 2.0')).toBe('Apache-2.0');
    expect(normalizeLicense('Apache-2.0')).toBe('Apache-2.0');
    expect(normalizeLicense('GPL-2.0')).toBe('GPL-2.0-only');
    expect(normalizeLicense('GPL-3.0')).toBe('GPL-3.0-only');
    expect(normalizeLicense('BSD-3-Clause')).toBe('BSD-3-Clause');
    expect(normalizeLicense('ISC')).toBe('ISC');
  });

  it('passes through unknown identifiers unchanged', () => {
    expect(normalizeLicense('LicenseRef-Custom')).toBe('LicenseRef-Custom');
  });

  it('returns NOASSERTION for empty input', () => {
    expect(normalizeLicense('')).toBe('NOASSERTION');
    expect(normalizeLicense('   ')).toBe('NOASSERTION');
  });
});

describe('classifyLicense', () => {
  it('classifies permissive licenses', () => {
    expect(classifyLicense('MIT')).toBe('permissive');
    expect(classifyLicense('Apache-2.0')).toBe('permissive');
    expect(classifyLicense('BSD-2-Clause')).toBe('permissive');
    expect(classifyLicense('ISC')).toBe('permissive');
    expect(classifyLicense('0BSD')).toBe('permissive');
  });

  it('classifies copyleft-weak licenses', () => {
    expect(classifyLicense('LGPL-2.1-only')).toBe('copyleft-weak');
    expect(classifyLicense('LGPL-3.0-or-later')).toBe('copyleft-weak');
    expect(classifyLicense('MPL-2.0')).toBe('copyleft-weak');
    expect(classifyLicense('EPL-2.0')).toBe('copyleft-weak');
  });

  it('classifies copyleft-strong licenses', () => {
    expect(classifyLicense('GPL-2.0-only')).toBe('copyleft-strong');
    expect(classifyLicense('GPL-3.0-only')).toBe('copyleft-strong');
    expect(classifyLicense('GPL-3.0-or-later')).toBe('copyleft-strong');
    expect(classifyLicense('AGPL-3.0-only')).toBe('copyleft-strong');
  });

  it('classifies proprietary licenses', () => {
    expect(classifyLicense('BUSL-1.1')).toBe('proprietary');
    expect(classifyLicense('UNLICENSED')).toBe('proprietary');
  });

  it('returns unknown for NOASSERTION and empty', () => {
    expect(classifyLicense('NOASSERTION')).toBe('unknown');
    expect(classifyLicense('')).toBe('unknown');
    expect(classifyLicense('NONE')).toBe('unknown');
  });

  it('returns unknown for LicenseRef-* custom references', () => {
    expect(classifyLicense('LicenseRef-Custom')).toBe('unknown');
    expect(classifyLicense('LicenseRef-scancode-proprietary-license')).toBe('unknown');
  });
});
