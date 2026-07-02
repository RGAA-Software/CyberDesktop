import json
import subprocess
from pathlib import Path


def repo_root(script_dir: Path) -> Path:
    return script_dir.parent.parent


def load_config(config_path: Path) -> dict:
    if config_path.is_file():
        return json.loads(config_path.read_text(encoding="utf-8"))
    return {}


def find_existing_path(candidates: list[Path], description: str) -> Path:
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise RuntimeError(f"Cannot find {description}. Checked: {candidates}")


def find_7z(config: dict, script_dir: Path) -> Path:
    root = repo_root(script_dir)
    configured = config.get("7z_path")
    candidates: list[Path] = []
    if configured:
        candidates.append((script_dir / configured).resolve())
    candidates.extend(
        [
            root / "tools" / "7z" / "7za.exe",
            Path(r"C:\Program Files\7-Zip\7z.exe"),
            Path(r"C:\Program Files (x86)\7-Zip\7z.exe"),
        ]
    )
    return find_existing_path(candidates, "7-Zip executable")


def find_makensis(config: dict, script_dir: Path) -> Path:
    root = repo_root(script_dir)
    configured = config.get("nsis_path")
    candidates: list[Path] = []
    if configured:
        candidates.append((script_dir / configured).resolve())
    candidates.extend(
        [
            root / "tools" / "nsis" / "makensis.exe",
            Path(r"C:\Program Files (x86)\NSIS\makensis.exe"),
            Path(r"C:\Program Files\NSIS\makensis.exe"),
        ]
    )
    return find_existing_path(candidates, "NSIS makensis.exe")


def read_workspace_version(workspace_dir: Path) -> str:
    import re

    cargo_toml = workspace_dir / "Cargo.toml"
    content = cargo_toml.read_text(encoding="utf-8")
    match = re.search(r"(?ms)^\[workspace\.package\].*?^version\s*=\s*\"([^\"]+)\"", content)
    if not match:
        raise RuntimeError(f"Cannot read workspace version from {cargo_toml}")
    return match.group(1)


def run_checked(cmd: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    print(f"Running: {' '.join(cmd)}")
    subprocess.run(cmd, cwd=str(cwd), check=True, env=env)


def build_archive(seven_zip: Path, stage_dir: Path, archive_path: Path) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    if archive_path.exists():
        archive_path.unlink()
    run_checked(
        [str(seven_zip), "a", "-t7z", str(archive_path), str(stage_dir / "*")],
        stage_dir,
    )


def build_nsis(
    makensis: Path,
    nsi_script: Path,
    output_dir: Path,
    version: str,
    installer_basename: str,
) -> None:
    run_checked(
        [
            str(makensis),
            f"/DOUTPUT_DIR={output_dir}",
            f"/DPRODUCT_VERSION={version}",
            f"/DINSTALLER_BASENAME={installer_basename}",
            str(nsi_script),
        ],
        nsi_script.parent,
    )
