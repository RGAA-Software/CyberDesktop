#!/usr/bin/env python3
"""Generate files-ui file_type_icons.rs from Zed icon_theme mappings + Tabler overrides."""

from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
ZED_ICON_THEME = REPO.parent / "zed" / "crates" / "theme" / "src" / "icon_theme.rs"
OUT = REPO / "crates" / "files-ui" / "src" / "file_type_icons.rs"
FILE_ICONS_DIR = REPO / "crates" / "app-assets" / "assets" / "icons" / "file_icons"

# Tabler outline file-type icons (see scripts/sync_tabler_icons.py).
TABLER_OVERRIDES: dict[str, str] = {
    "pdf": "icons/tabler/file-type-pdf.svg",
    "htm": "icons/tabler/file-type-html.svg",
    "html": "icons/tabler/file-type-html.svg",
    "js": "icons/tabler/file-type-js.svg",
    "mjs": "icons/tabler/file-type-js.svg",
    "cjs": "icons/tabler/file-type-js.svg",
    "ts": "icons/tabler/file-type-ts.svg",
    "tsx": "icons/tabler/file-type-ts.svg",
    "mts": "icons/tabler/file-type-ts.svg",
    "cts": "icons/tabler/file-type-ts.svg",
    "css": "icons/tabler/file-type-css.svg",
    "pcss": "icons/tabler/file-type-css.svg",
    "postcss": "icons/tabler/file-type-css.svg",
    "xml": "icons/tabler/file-type-xml.svg",
    "sql": "icons/tabler/file-type-sql.svg",
    "sqlite": "icons/tabler/file-type-sql.svg",
    "doc": "icons/tabler/file-type-doc.svg",
    "docx": "icons/tabler/file-type-doc.svg",
    "xls": "icons/tabler/file-type-xls.svg",
    "xlsx": "icons/tabler/file-type-xls.svg",
    "ppt": "icons/tabler/file-type-ppt.svg",
    "pptx": "icons/tabler/file-type-ppt.svg",
    "zip": "icons/tabler/file-type-zip.svg",
    "rar": "icons/tabler/file-type-zip.svg",
    "7z": "icons/tabler/file-type-zip.svg",
    "tar": "icons/tabler/file-type-zip.svg",
    "gz": "icons/tabler/file-type-zip.svg",
    "csv": "icons/tabler/file-type-csv.svg",
    "tsv": "icons/tabler/file-type-csv.svg",
    "jpg": "icons/tabler/file-type-jpg.svg",
    "jpeg": "icons/tabler/file-type-jpg.svg",
    "png": "icons/tabler/file-type-png.svg",
    "bmp": "icons/tabler/file-type-bmp.svg",
    "svg": "icons/tabler/file-type-svg.svg",
    "vue": "icons/tabler/file-type-vue.svg",
    "rs": "icons/tabler/file-type-rs.svg",
    "php": "icons/tabler/file-type-php.svg",
    "txt": "icons/tabler/file-type-txt.svg",
    "cpp": "icons/tabler/file-type-cpp.svg",
    "cc": "icons/tabler/file-type-cpp.svg",
    "cxx": "icons/tabler/file-type-cpp.svg",
    "hpp": "icons/tabler/file-type-cpp.svg",
    "c++": "icons/tabler/file-type-cpp.svg",
    "h++": "icons/tabler/file-type-cpp.svg",
    "hh": "icons/tabler/file-type-cpp.svg",
    "hxx": "icons/tabler/file-type-cpp.svg",
    "inl": "icons/tabler/file-type-cpp.svg",
    "ixx": "icons/tabler/file-type-cpp.svg",
    "cppm": "icons/tabler/file-type-cpp.svg",
}

# Common formats on Windows/desktop — Tabler icons (https://tabler.io/icons).
# Used when Zed mapping is missing or for clearer category icons.
COMMON_TABLER_OVERRIDES: dict[str, str] = {
    # Audio → file-music
    "aac": "icons/tabler/file-music.svg",
    "aif": "icons/tabler/file-music.svg",
    "aiff": "icons/tabler/file-music.svg",
    "flac": "icons/tabler/file-music.svg",
    "m4a": "icons/tabler/file-music.svg",
    "mid": "icons/tabler/file-music.svg",
    "midi": "icons/tabler/file-music.svg",
    "mka": "icons/tabler/file-music.svg",
    "mp3": "icons/tabler/file-music.svg",
    "ogg": "icons/tabler/file-music.svg",
    "opus": "icons/tabler/file-music.svg",
    "wav": "icons/tabler/file-music.svg",
    "weba": "icons/tabler/file-music.svg",
    "wma": "icons/tabler/file-music.svg",
    "wv": "icons/tabler/file-music.svg",
    # Video → video
    "avi": "icons/tabler/video.svg",
    "flv": "icons/tabler/video.svg",
    "m4v": "icons/tabler/video.svg",
    "mkv": "icons/tabler/video.svg",
    "mov": "icons/tabler/video.svg",
    "mp4": "icons/tabler/video.svg",
    "mpg": "icons/tabler/video.svg",
    "mpeg": "icons/tabler/video.svg",
    "webm": "icons/tabler/video.svg",
    "wmv": "icons/tabler/video.svg",
    # Binaries / libraries / installers → binary
    "apk": "icons/tabler/binary.svg",
    "app": "icons/tabler/binary.svg",
    "bin": "icons/tabler/binary.svg",
    "com": "icons/tabler/binary.svg",
    "cpl": "icons/tabler/binary.svg",
    "deb": "icons/tabler/binary.svg",
    "dll": "icons/tabler/binary.svg",
    "drv": "icons/tabler/binary.svg",
    "dylib": "icons/tabler/binary.svg",
    "exe": "icons/tabler/binary.svg",
    "img": "icons/tabler/binary.svg",
    "iso": "icons/tabler/binary.svg",
    "msi": "icons/tabler/binary.svg",
    "ocx": "icons/tabler/binary.svg",
    "rpm": "icons/tabler/binary.svg",
    "scr": "icons/tabler/binary.svg",
    "so": "icons/tabler/binary.svg",
    "sys": "icons/tabler/binary.svg",
    "vhd": "icons/tabler/binary.svg",
    "vhdx": "icons/tabler/binary.svg",
    # Images → photo
    "avif": "icons/tabler/photo.svg",
    "cr2": "icons/tabler/photo.svg",
    "gif": "icons/tabler/photo.svg",
    "heic": "icons/tabler/photo.svg",
    "heif": "icons/tabler/photo.svg",
    "ico": "icons/tabler/photo.svg",
    "nef": "icons/tabler/photo.svg",
    "raw": "icons/tabler/photo.svg",
    "tif": "icons/tabler/photo.svg",
    "tiff": "icons/tabler/photo.svg",
    "webp": "icons/tabler/photo.svg",
    # Office / documents
    "docx": "icons/tabler/file-type-docx.svg",
    "odp": "icons/tabler/file-type-ppt.svg",
    "ods": "icons/tabler/file-type-xls.svg",
    "odt": "icons/tabler/file-type-doc.svg",
    "rtf": "icons/tabler/file-type-doc.svg",
    # Data / databases
    "accdb": "icons/tabler/database.svg",
    "dat": "icons/tabler/database.svg",
    "db": "icons/tabler/database.svg",
    "dbf": "icons/tabler/database.svg",
    "mdb": "icons/tabler/database.svg",
    "sdf": "icons/tabler/database.svg",
    # Shell / scripts
    "bash": "icons/tabler/terminal-2.svg",
    "bat": "icons/tabler/terminal-2.svg",
    "cmd": "icons/tabler/terminal-2.svg",
    "fish": "icons/tabler/terminal-2.svg",
    "nu": "icons/tabler/terminal-2.svg",
    "ps1": "icons/tabler/terminal-2.svg",
    "psm1": "icons/tabler/terminal-2.svg",
    "sh": "icons/tabler/terminal-2.svg",
    "zsh": "icons/tabler/terminal-2.svg",
    # Text / config / logs
    "cfg": "icons/tabler/file-settings.svg",
    "conf": "icons/tabler/file-settings.svg",
    "ini": "icons/tabler/file-settings.svg",
    "log": "icons/tabler/file-text.svg",
    "markdown": "icons/tabler/file-text.svg",
    "md": "icons/tabler/file-text.svg",
    "reg": "icons/tabler/file-settings.svg",
    # Code / data interchange
    "json": "icons/tabler/file-code.svg",
    "jsonc": "icons/tabler/file-code.svg",
    "yaml": "icons/tabler/file-code.svg",
    "yml": "icons/tabler/file-code.svg",
    # Ebooks
    "azw": "icons/tabler/book.svg",
    "azw3": "icons/tabler/book.svg",
    "epub": "icons/tabler/book.svg",
    "mobi": "icons/tabler/book.svg",
    # Shortcuts / certs
    "lnk": "icons/tabler/link.svg",
    "cer": "icons/tabler/file-certificate.svg",
    "crt": "icons/tabler/file-certificate.svg",
    "p12": "icons/tabler/file-certificate.svg",
    "pem": "icons/tabler/file-certificate.svg",
    "pfx": "icons/tabler/file-certificate.svg",
}


def all_tabler_overrides() -> dict[str, str]:
    merged = dict(COMMON_TABLER_OVERRIDES)
    merged.update(TABLER_OVERRIDES)
    return merged


def parse_zed_arrays(text: str) -> tuple[list[tuple[str, list[str]]], dict[str, str]]:
    suffix_block = re.search(
        r"const FILE_SUFFIXES_BY_ICON_KEY.*?=\s*&\[(.*)\]\s*;\s*\n\s*/// A mapping of a file type",
        text,
        re.DOTALL,
    )
    icons_block = re.search(
        r"const FILE_ICONS.*?=\s*&\[(.*)\]\s*;\s*\n\s*/// Returns a mapping",
        text,
        re.DOTALL,
    )
    if not suffix_block or not icons_block:
        raise SystemExit("failed to parse Zed icon_theme.rs")

    suffixes: list[tuple[str, list[str]]] = []
    # Allow `(\n        "key",\n        &[` multiline entries from Zed icon_theme.rs.
    for key, exts_raw in re.findall(
        r'\(\s*"([^"]+)"\s*,\s*&\[([^\]]*)\]\s*,?\s*\)',
        suffix_block.group(1),
        re.DOTALL,
    ):
        exts = re.findall(r'"([^"]+)"', exts_raw)
        suffixes.append((key, exts))

    icons: dict[str, str] = {}
    for key, path in re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', icons_block.group(1)):
        icons[key] = path

    return suffixes, icons


def normalize_zed_svgs() -> None:
    if not FILE_ICONS_DIR.is_dir():
        return
    for path in FILE_ICONS_DIR.glob("*.svg"):
        text = path.read_text(encoding="utf-8")
        updated = (
            text.replace('stroke="black"', 'stroke="currentColor"')
            .replace("stroke='black'", "stroke='currentColor'")
            .replace('fill="black"', 'fill="currentColor"')
            .replace("fill='black'", "fill='currentColor'")
        )
        if updated != text:
            path.write_text(updated, encoding="utf-8", newline="\n")


def emit(suffixes: list[tuple[str, list[str]]], icons: dict[str, str]) -> None:
    lines: list[str] = [
        "//! Extension and path → bundled SVG icon paths (Tabler + Zed file_icons).",
        "//!",
        "//! Generated by `python scripts/generate_file_type_icons.py`. Do not edit by hand.",
        "",
        "use std::collections::HashMap;",
        "use std::path::Path;",
        "use std::sync::OnceLock;",
        "",
        "use files_fs::FileItemKind;",
        "",
        "use crate::tabler_icons;",
        "",
        "const DEFAULT_FILE: &str = \"icons/file_icons/file.svg\";",
        "const DEFAULT_FOLDER: &str = tabler_icons::FOLDER;",
        "const DEFAULT_SYMLINK: &str = tabler_icons::LINK;",
        "",
        "pub const FALLBACK_FILE_ICON: &str = DEFAULT_FILE;",
        "",
        "fn tabler_overrides() -> &'static HashMap<&'static str, &'static str> {",
        "    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();",
        "    MAP.get_or_init(|| HashMap::from([",
    ]
    for ext, path in sorted(all_tabler_overrides().items()):
        lines.append(f'        ("{ext}", "{path}"),')
    lines.extend(
        [
            "    ]))",
            "}",
            "",
            "fn zed_icon_paths() -> &'static HashMap<&'static str, &'static str> {",
            "    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();",
            "    MAP.get_or_init(|| HashMap::from([",
        ]
    )
    for key, path in sorted(icons.items()):
        lines.append(f'        ("{key}", "{path}"),')
    lines.extend(
        [
            "    ]))",
            "}",
            "",
            "fn extension_to_icon_key() -> &'static HashMap<&'static str, &'static str> {",
            "    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();",
            "    MAP.get_or_init(|| {",
            "        let mut map = HashMap::new();",
        ]
    )
    for key, exts in suffixes:
        for ext in exts:
            lines.append(f'        map.insert("{ext}", "{key}");')
    lines.extend(
        [
            "        map",
            "    })",
            "}",
            "",
            "fn icon_path_for_key(key: &str) -> &'static str {",
            "    zed_icon_paths().get(key).copied().unwrap_or(DEFAULT_FILE)",
            "}",
            "",
            "/// Resolve a file suffix (extension, compound suffix, or file name) to an SVG asset path.",
            "pub fn svg_path_for_suffix(suffix: &str) -> &'static str {",
            "    let suffix = suffix.trim();",
            "    if suffix.is_empty() {",
            "        return DEFAULT_FILE;",
            "    }",
            "    let lower = suffix.to_ascii_lowercase();",
            "    if let Some(path) = tabler_overrides().get(lower.as_str()) {",
            "        return path;",
            "    }",
            "    if let Some(key) = extension_to_icon_key().get(lower.as_str()) {",
            "        return icon_path_for_key(key);",
            "    }",
            "    DEFAULT_FILE",
            "}",
            "",
            "/// Extension without leading dot, e.g. `\"pdf\"`.",
            "pub fn svg_path_for_extension(ext: &str) -> &'static str {",
            "    svg_path_for_suffix(ext)",
            "}",
            "",
            "fn suffix_candidates(path: &Path) -> Vec<String> {",
            "    let mut out = Vec::new();",
            "    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {",
            "        out.push(name.to_string());",
            "        let mut rest = name;",
            "        while let Some((_, suffix)) = rest.split_once('.') {",
            "            out.push(suffix.to_string());",
            "            rest = suffix;",
            "        }",
            "    }",
            "    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {",
            "        out.push(ext.to_string());",
            "    }",
            "    out",
            "}",
            "",
            "/// Best bundled icon for a filesystem path (folder, symlink, drive root, or file).",
            "pub fn svg_path_for_path(path: &Path) -> &'static str {",
            "    if path.as_os_str().is_empty() {",
            "        return DEFAULT_FOLDER;",
            "    }",
            "    #[cfg(windows)]",
            "    {",
            "        let s = path.to_string_lossy();",
            "        if s.len() == 3 && s.as_bytes()[1] == b':' && s.as_bytes()[2] == b'\\\\' {",
            "            if s.starts_with('C') || s.starts_with('c') {",
            "                return tabler_icons::BRAND_WINDOWS;",
            "            }",
            "            return tabler_icons::DATABASE;",
            "        }",
            "    }",
            "    if path.is_dir() {",
            "        return DEFAULT_FOLDER;",
            "    }",
            "    if path.is_symlink() {",
            "        return DEFAULT_SYMLINK;",
            "    }",
            "    for suffix in suffix_candidates(path) {",
            "        let candidate = svg_path_for_suffix(&suffix);",
            "        if candidate != DEFAULT_FILE {",
            "            return candidate;",
            "        }",
            "    }",
            "    DEFAULT_FILE",
            "}",
            "",
            "pub fn svg_path_for_kind_and_extension(kind: FileItemKind, extension: Option<&str>) -> &'static str {",
            "    match kind {",
            "        FileItemKind::Folder => DEFAULT_FOLDER,",
            "        FileItemKind::Symlink => DEFAULT_SYMLINK,",
            "        _ => extension",
            "            .filter(|e| !e.is_empty())",
            "            .map(svg_path_for_extension)",
            "            .unwrap_or(DEFAULT_FILE),",
            "    }",
            "}",
            "",
        ]
    )
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def main() -> None:
    text = ZED_ICON_THEME.read_text(encoding="utf-8")
    suffixes, icons = parse_zed_arrays(text)
    normalize_zed_svgs()
    emit(suffixes, icons)
    overrides = all_tabler_overrides()
    ext_count = sum(len(exts) for _, exts in suffixes)
    print(
        f"Wrote {OUT} ({len(suffixes)} Zed icon keys, {ext_count} Zed suffixes, "
        f"{len(overrides)} Tabler overrides)"
    )


if __name__ == "__main__":
    main()
