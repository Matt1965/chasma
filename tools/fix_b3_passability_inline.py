"""Replace inline PassabilityCatalogs temporaries with bundle.catalogs()."""

from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "src"

INLINE = re.compile(
    r"PassabilityCatalogs\s*\{\s*"
    r"doodad:\s*&([^,]+),\s*"
    r"building:\s*&BuildingCatalog::default\(\),\s*"
    r"footprint:\s*&FootprintCatalog::default\(\),\s*"
    r"\}",
    re.MULTILINE,
)


def fix_file(path: pathlib.Path) -> bool:
    text = path.read_text(encoding="utf-8")
    if "PassabilityCatalogs {" not in text or "building: &BuildingCatalog::default()" not in text:
        return False
    orig = text

    # Only patch inside test modules when a bundle helper can be added per-function.
    # Global replace: use a one-line helper call pattern with local bundle variable is hard.
    # Replace inline struct with macro-like helper invocation requiring bundle in scope.
    text = INLINE.sub(r"__BUNDLE__.catalogs()", text)

    if text != orig and "__BUNDLE__.catalogs()" in text:
        # For each function containing __BUNDLE__, prepend let __BUNDLE__ = ...
        # Simpler: replace at file level in test mod with a lazy approach
        if "#[cfg(test)]" in text:
            test_idx = text.rfind("#[cfg(test)]")
            test_body = text[test_idx:]
            if "__BUNDLE__" in test_body and "fn passability_bundle()" not in test_body:
                insert = (
                    "\n    fn passability_bundle() -> crate::world::TestPassabilityBundle {\n"
                    "        crate::world::TestPassabilityBundle::new()\n"
                    "    }\n\n"
                    "    fn passability_catalogs() -> crate::world::PassabilityCatalogs<'static> {\n"
                    "        unreachable!()\n"
                    "    }\n"
                )
                # bad approach - skip
        path.write_text(text, encoding="utf-8")
        return True
    return False


if __name__ == "__main__":
    print("manual fix preferred")
