/// License normalization: map raw/variant identifiers → SPDX canonical form.
/// License classification: assign a `LicenseClass` based on the SPDX ID.
use crate::LicenseClass;

/// Normalize a raw license string to a canonical SPDX identifier.
///
/// Handles common variant spellings, casing differences, and shorthand forms.
/// Returns the input unchanged (trimmed) if no mapping is found.
pub fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();
    let lower = lower.as_str();

    // Strip common wrapper phrases
    let lower = lower
        .trim_start_matches("see license in ")
        .trim_start_matches("see ")
        .trim();

    match lower {
        // MIT variants
        "mit" | "mit license" | "mit-license" | "mit licensed" => "MIT",
        "mit-0" => "MIT-0",
        "x11" => "X11",

        // Apache variants
        "apache-2.0"
        | "apache 2.0"
        | "apache2"
        | "apache2.0"
        | "apache license 2.0"
        | "apache license, version 2.0"
        | "apache software license 2.0" => "Apache-2.0",
        "apache-1.1" | "apache 1.1" => "Apache-1.1",
        "apache-1.0" | "apache 1.0" => "Apache-1.0",

        // BSD variants
        "bsd" | "bsd license" => "BSD-2-Clause",
        "bsd-2-clause" | "bsd 2-clause" | "bsd-2" | "simplified bsd" | "freebsd" | "bsd 2" => {
            "BSD-2-Clause"
        }
        "bsd-3-clause" | "bsd 3-clause" | "bsd-3" | "new bsd" | "modified bsd" | "bsd 3" => {
            "BSD-3-Clause"
        }
        "bsd-4-clause" | "bsd 4-clause" | "original bsd" => "BSD-4-Clause",
        "0bsd" | "zero-clause bsd" => "0BSD",

        // ISC
        "isc" | "isc license" => "ISC",

        // Unlicense / public domain
        "unlicense" | "the unlicense" => "Unlicense",
        "cc0" | "cc0-1.0" | "cc0 1.0" | "public domain" | "cc0 universal" => "CC0-1.0",
        "wtfpl" => "WTFPL",

        // LGPL variants
        "lgpl-2.0" | "lgpl 2.0" | "gnu lesser general public license v2" => "LGPL-2.0-only",
        "lgpl-2.0+" | "lgpl-2.0-or-later" | "lgpl v2+" => "LGPL-2.0-or-later",
        "lgpl-2.1" | "lgpl 2.1" | "gnu lesser general public license v2.1" | "lgpl2.1" => {
            "LGPL-2.1-only"
        }
        "lgpl-2.1+" | "lgpl-2.1-or-later" | "lgpl v2.1+" | "lgpl2.1+" => "LGPL-2.1-or-later",
        "lgpl-3.0" | "lgpl 3.0" | "gnu lesser general public license v3" | "lgpl3" => {
            "LGPL-3.0-only"
        }
        "lgpl-3.0+" | "lgpl-3.0-or-later" | "lgpl v3+" | "lgpl3+" => "LGPL-3.0-or-later",

        // GPL variants
        "gpl-2.0"
        | "gpl 2.0"
        | "gplv2"
        | "gpl2"
        | "gnu general public license v2"
        | "gnu gpl v2" => "GPL-2.0-only",
        "gpl-2.0+" | "gpl-2.0-or-later" | "gplv2+" | "gpl v2+" => "GPL-2.0-or-later",
        "gpl-3.0"
        | "gpl 3.0"
        | "gplv3"
        | "gpl3"
        | "gnu general public license v3"
        | "gnu gpl v3" => "GPL-3.0-only",
        "gpl-3.0+" | "gpl-3.0-or-later" | "gplv3+" | "gpl v3+" => "GPL-3.0-or-later",

        // AGPL
        "agpl-3.0" | "agpl 3.0" | "agplv3" | "gnu affero general public license v3" => {
            "AGPL-3.0-only"
        }
        "agpl-3.0+" | "agpl-3.0-or-later" | "agplv3+" => "AGPL-3.0-or-later",

        // MPL (Mozilla)
        "mpl-1.0" | "mpl 1.0" | "mozilla public license 1.0" => "MPL-1.0",
        "mpl-1.1" | "mpl 1.1" | "mozilla public license 1.1" => "MPL-1.1",
        "mpl-2.0" | "mpl 2.0" | "mozilla public license 2.0" | "mpl2" => "MPL-2.0",

        // CDDL
        "cddl-1.0" | "cddl 1.0" | "cddl" => "CDDL-1.0",

        // EPL (Eclipse)
        "epl-1.0" | "epl 1.0" | "eclipse public license 1.0" | "epl" => "EPL-1.0",
        "epl-2.0" | "epl 2.0" | "eclipse public license 2.0" => "EPL-2.0",

        // EUPL
        "eupl-1.1" | "eupl 1.1" => "EUPL-1.1",
        "eupl-1.2" | "eupl 1.2" => "EUPL-1.2",

        // Artistic
        "artistic-1.0" | "artistic 1.0" | "artistic" => "Artistic-1.0",
        "artistic-2.0" | "artistic 2.0" => "Artistic-2.0",

        // CPAL
        "cpal-1.0" | "cpal" => "CPAL-1.0",

        // Proprietary / non-FOSS markers
        "proprietary"
        | "commercial"
        | "all rights reserved"
        | "closed source"
        | "see license"
        | "see license file"
        | "unlicensed"
        | "none" => "LicenseRef-Proprietary",

        // Python Software Foundation
        "psf" | "psf license" | "python software foundation" | "psfl" => "PSF-2.0",

        // Boost
        "boost" | "boost software license" | "bsl-1.0" | "bsl 1.0" => "BSL-1.0",

        // Zlib
        "zlib" | "zlib license" => "Zlib",

        // Ruby
        "ruby" | "ruby license" => "Ruby",

        _ => return trimmed.to_string(),
    }
    .to_string()
}

/// Classify a normalized SPDX license identifier into a high-level bucket.
pub fn classify(spdx_id: &str) -> LicenseClass {
    match spdx_id {
        // ---------- Permissive ----------
        "MIT" | "MIT-0" | "X11" | "Apache-2.0" | "Apache-1.1" | "Apache-1.0" | "BSD-2-Clause"
        | "BSD-3-Clause" | "BSD-4-Clause" | "0BSD" | "ISC" | "Unlicense" | "CC0-1.0" | "WTFPL"
        | "PSF-2.0" | "BSL-1.0" | "Zlib" | "Ruby" => LicenseClass::Permissive,

        // ---------- Copyleft weak (library/file-level) ----------
        "LGPL-2.0-only" | "LGPL-2.0-or-later" | "LGPL-2.1-only" | "LGPL-2.1-or-later"
        | "LGPL-3.0-only" | "LGPL-3.0-or-later" | "MPL-1.0" | "MPL-1.1" | "MPL-2.0"
        | "CDDL-1.0" | "EPL-1.0" | "EPL-2.0" | "EUPL-1.1" | "EUPL-1.2" | "CPAL-1.0"
        | "Artistic-1.0" | "Artistic-2.0" => LicenseClass::CopyleftWeak,

        // ---------- Copyleft strong (project-level) ----------
        "GPL-2.0-only" | "GPL-2.0-or-later" | "GPL-3.0-only" | "GPL-3.0-or-later"
        | "AGPL-3.0-only" | "AGPL-3.0-or-later" => LicenseClass::CopyleftStrong,

        // ---------- Proprietary ----------
        id if id.starts_with("LicenseRef-Proprietary") => LicenseClass::Proprietary,

        _ => LicenseClass::Unknown,
    }
}

/// Convenience: normalize then classify a raw license string.
pub fn normalize_and_classify(raw: &str) -> (String, LicenseClass) {
    let spdx = normalize(raw);
    let class = classify(&spdx);
    (spdx, class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_mit_variants() {
        assert_eq!(normalize("MIT"), "MIT");
        assert_eq!(normalize("mit"), "MIT");
        assert_eq!(normalize("MIT License"), "MIT");
        assert_eq!(normalize("  MIT  "), "MIT");
    }

    #[test]
    fn normalize_apache_variants() {
        assert_eq!(normalize("Apache-2.0"), "Apache-2.0");
        assert_eq!(normalize("Apache 2.0"), "Apache-2.0");
        assert_eq!(normalize("apache2"), "Apache-2.0");
        assert_eq!(normalize("Apache License 2.0"), "Apache-2.0");
    }

    #[test]
    fn normalize_gpl_variants() {
        assert_eq!(normalize("GPLv2"), "GPL-2.0-only");
        assert_eq!(normalize("gpl 3.0"), "GPL-3.0-only");
        assert_eq!(normalize("GPL-3.0+"), "GPL-3.0-or-later");
    }

    #[test]
    fn normalize_lgpl_variants() {
        assert_eq!(normalize("LGPL-2.1"), "LGPL-2.1-only");
        assert_eq!(normalize("LGPL2.1+"), "LGPL-2.1-or-later");
    }

    #[test]
    fn normalize_bsd_variants() {
        assert_eq!(normalize("BSD"), "BSD-2-Clause");
        assert_eq!(normalize("BSD-3"), "BSD-3-Clause");
        assert_eq!(normalize("New BSD"), "BSD-3-Clause");
        assert_eq!(normalize("0BSD"), "0BSD");
    }

    #[test]
    fn normalize_proprietary() {
        assert_eq!(normalize("proprietary"), "LicenseRef-Proprietary");
        assert_eq!(normalize("All Rights Reserved"), "LicenseRef-Proprietary");
        assert_eq!(normalize("UNLICENSED"), "LicenseRef-Proprietary");
    }

    #[test]
    fn normalize_passthrough_unknown() {
        assert_eq!(normalize("MyCustomLicense-1.0"), "MyCustomLicense-1.0");
    }

    #[test]
    fn classify_permissive() {
        assert_eq!(classify("MIT"), LicenseClass::Permissive);
        assert_eq!(classify("Apache-2.0"), LicenseClass::Permissive);
        assert_eq!(classify("ISC"), LicenseClass::Permissive);
        assert_eq!(classify("BSD-3-Clause"), LicenseClass::Permissive);
        assert_eq!(classify("0BSD"), LicenseClass::Permissive);
        assert_eq!(classify("CC0-1.0"), LicenseClass::Permissive);
    }

    #[test]
    fn classify_copyleft_weak() {
        assert_eq!(classify("LGPL-2.1-only"), LicenseClass::CopyleftWeak);
        assert_eq!(classify("MPL-2.0"), LicenseClass::CopyleftWeak);
        assert_eq!(classify("EPL-2.0"), LicenseClass::CopyleftWeak);
    }

    #[test]
    fn classify_copyleft_strong() {
        assert_eq!(classify("GPL-2.0-only"), LicenseClass::CopyleftStrong);
        assert_eq!(classify("GPL-3.0-or-later"), LicenseClass::CopyleftStrong);
        assert_eq!(classify("AGPL-3.0-only"), LicenseClass::CopyleftStrong);
    }

    #[test]
    fn classify_proprietary() {
        assert_eq!(
            classify("LicenseRef-Proprietary"),
            LicenseClass::Proprietary
        );
    }

    #[test]
    fn classify_unknown() {
        assert_eq!(classify("MyCustomLicense-1.0"), LicenseClass::Unknown);
    }

    #[test]
    fn normalize_and_classify_roundtrip() {
        let (spdx, class) = normalize_and_classify("MIT");
        assert_eq!(spdx, "MIT");
        assert_eq!(class, LicenseClass::Permissive);

        let (spdx, class) = normalize_and_classify("GPLv3+");
        assert_eq!(spdx, "GPL-3.0-or-later");
        assert_eq!(class, LicenseClass::CopyleftStrong);
    }
}
