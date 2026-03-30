import type { LicenseCategory } from './types.js';

/**
 * Normalize common non-SPDX license strings to their canonical SPDX identifiers.
 * Keys are lowercased for case-insensitive lookup.
 */
const NORMALIZE_MAP: Record<string, string> = {
  // MIT variants
  'mit': 'MIT',
  'mit license': 'MIT',
  'the mit license': 'MIT',
  // Apache variants
  'apache 2': 'Apache-2.0',
  'apache 2.0': 'Apache-2.0',
  'apache-2': 'Apache-2.0',
  'apache-2.0': 'Apache-2.0',
  'apache license 2.0': 'Apache-2.0',
  'apache license, version 2.0': 'Apache-2.0',
  'apache software license': 'Apache-2.0',
  // BSD variants
  'bsd': 'BSD-2-Clause',
  'bsd-2': 'BSD-2-Clause',
  'bsd-2-clause': 'BSD-2-Clause',
  'bsd 2-clause': 'BSD-2-Clause',
  'simplified bsd': 'BSD-2-Clause',
  'bsd-3': 'BSD-3-Clause',
  'bsd-3-clause': 'BSD-3-Clause',
  'bsd 3-clause': 'BSD-3-Clause',
  'new bsd': 'BSD-3-Clause',
  'modified bsd': 'BSD-3-Clause',
  // GPL variants
  'gpl': 'GPL-3.0-only',
  'gpl-2': 'GPL-2.0-only',
  'gpl-2.0': 'GPL-2.0-only',
  'gpl2': 'GPL-2.0-only',
  'gnu gpl v2': 'GPL-2.0-only',
  'gpl-2.0+': 'GPL-2.0-or-later',
  'gpl-2.0-only': 'GPL-2.0-only',
  'gpl-2.0-or-later': 'GPL-2.0-or-later',
  'gpl-3': 'GPL-3.0-only',
  'gpl-3.0': 'GPL-3.0-only',
  'gpl3': 'GPL-3.0-only',
  'gnu gpl v3': 'GPL-3.0-only',
  'gpl-3.0+': 'GPL-3.0-or-later',
  'gpl-3.0-only': 'GPL-3.0-only',
  'gpl-3.0-or-later': 'GPL-3.0-or-later',
  // LGPL variants
  'lgpl': 'LGPL-2.1-only',
  'lgpl-2.0': 'LGPL-2.0-only',
  'lgpl-2.0-only': 'LGPL-2.0-only',
  'lgpl-2.1': 'LGPL-2.1-only',
  'lgpl-2.1-only': 'LGPL-2.1-only',
  'lgpl-2.1+': 'LGPL-2.1-or-later',
  'lgpl-2.1-or-later': 'LGPL-2.1-or-later',
  'lgpl-3.0': 'LGPL-3.0-only',
  'lgpl-3.0-only': 'LGPL-3.0-only',
  'lgpl-3.0+': 'LGPL-3.0-or-later',
  'lgpl-3.0-or-later': 'LGPL-3.0-or-later',
  // AGPL variants
  'agpl': 'AGPL-3.0-only',
  'agpl-3.0': 'AGPL-3.0-only',
  'agpl-3.0-only': 'AGPL-3.0-only',
  'agpl-3.0+': 'AGPL-3.0-or-later',
  'agpl-3.0-or-later': 'AGPL-3.0-or-later',
  // Mozilla
  'mpl': 'MPL-2.0',
  'mpl-2.0': 'MPL-2.0',
  'mozilla public license 2.0': 'MPL-2.0',
  // ISC
  'isc': 'ISC',
  'isc license': 'ISC',
  // Other permissive
  '0bsd': '0BSD',
  'zlib': 'Zlib',
  'unlicense': 'Unlicense',
  'the unlicense': 'Unlicense',
  'public domain': 'Unlicense',
  'cc0': 'CC0-1.0',
  'cc0-1.0': 'CC0-1.0',
  'creative commons zero': 'CC0-1.0',
  'wtfpl': 'WTFPL',
  'python': 'Python-2.0',
  'psfrag': 'Python-2.0',
  // Eclipse
  'epl': 'EPL-2.0',
  'epl-1.0': 'EPL-1.0',
  'epl-2.0': 'EPL-2.0',
  // Business Source
  'busl-1.1': 'BUSL-1.1',
  'business source license 1.1': 'BUSL-1.1',
};

/**
 * Map from SPDX identifier → license category.
 */
const CLASSIFICATION_MAP: Record<string, LicenseCategory> = {
  // Permissive
  'MIT': 'permissive',
  'Apache-2.0': 'permissive',
  'BSD-2-Clause': 'permissive',
  'BSD-3-Clause': 'permissive',
  'BSD-4-Clause': 'permissive',
  'ISC': 'permissive',
  '0BSD': 'permissive',
  'Zlib': 'permissive',
  'Unlicense': 'permissive',
  'CC0-1.0': 'permissive',
  'WTFPL': 'permissive',
  'Python-2.0': 'permissive',
  'PSF-2.0': 'permissive',
  'Artistic-2.0': 'permissive',
  'BlueOak-1.0.0': 'permissive',
  'curl': 'permissive',
  'OpenSSL': 'permissive',
  'Beerware': 'permissive',
  // Copyleft-weak
  'LGPL-2.0-only': 'copyleft-weak',
  'LGPL-2.0-or-later': 'copyleft-weak',
  'LGPL-2.1-only': 'copyleft-weak',
  'LGPL-2.1-or-later': 'copyleft-weak',
  'LGPL-3.0-only': 'copyleft-weak',
  'LGPL-3.0-or-later': 'copyleft-weak',
  'MPL-2.0': 'copyleft-weak',
  'MPL-1.1': 'copyleft-weak',
  'CDDL-1.0': 'copyleft-weak',
  'EPL-1.0': 'copyleft-weak',
  'EPL-2.0': 'copyleft-weak',
  'EUPL-1.1': 'copyleft-weak',
  'EUPL-1.2': 'copyleft-weak',
  'CECILL-2.1': 'copyleft-weak',
  // Copyleft-strong
  'GPL-2.0-only': 'copyleft-strong',
  'GPL-2.0-or-later': 'copyleft-strong',
  'GPL-3.0-only': 'copyleft-strong',
  'GPL-3.0-or-later': 'copyleft-strong',
  'AGPL-3.0-only': 'copyleft-strong',
  'AGPL-3.0-or-later': 'copyleft-strong',
  'OSL-3.0': 'copyleft-strong',
  // Proprietary / restrictive
  'BUSL-1.1': 'proprietary',
  'SSPL-1.0': 'proprietary',
  'proprietary': 'proprietary',
  'UNLICENSED': 'proprietary',
};

/**
 * Normalize a raw license string to a canonical SPDX identifier.
 * Returns the input unchanged if no mapping is found.
 */
export function normalizeLicense(raw: string): string {
  if (!raw || raw.trim() === '') return 'NOASSERTION';
  const lower = raw.trim().toLowerCase();
  return NORMALIZE_MAP[lower] ?? raw.trim();
}

/**
 * Classify a canonical SPDX identifier into a category.
 */
export function classifyLicense(spdxId: string): LicenseCategory {
  if (!spdxId || spdxId === 'NOASSERTION' || spdxId === 'NONE') return 'unknown';
  // Direct lookup
  const direct = CLASSIFICATION_MAP[spdxId];
  if (direct) return direct;
  // Heuristics for unknown identifiers
  const upper = spdxId.toUpperCase();
  if (upper.includes('GPL') && !upper.includes('LGPL')) return 'copyleft-strong';
  if (upper.includes('LGPL') || upper.includes('MPL') || upper.includes('EPL') || upper.includes('EUPL')) return 'copyleft-weak';
  if (upper === 'PROPRIETARY' || upper === 'UNLICENSED' || upper.startsWith('SEE LICENSE')) return 'proprietary';
  return 'unknown';
}
