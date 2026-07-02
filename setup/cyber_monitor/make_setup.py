import argparse
import os
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from common.packaging import (  # noqa: E402
    build_archive,
    build_nsis,
    find_7z,
    find_makensis,
    load_config,
    read_workspace_version,
    repo_root,
    run_checked,
)

PRODUCT_KEY = "cyber_monitor"
PRODUCT_NAME = "CyberMonitor Suite"
INSTALLER_BASENAME = "CyberMonitorSuite"
MONITOR_EXE = "cyber_monitor.exe"
HOST_EXE = "cyber_monitor_host.exe"


def build_release_binaries(workspace_dir: Path) -> None:
    env = os.environ.copy()
    native_flag = "-C target-cpu=native"
    env["RUSTFLAGS"] = (
        f"{env['RUSTFLAGS']} {native_flag}".strip() if env.get("RUSTFLAGS") else native_flag
    )
    for bin_name in ("cyber_monitor", "cyber_monitor_host"):
        run_checked(
            ["cargo", "build", "-p", "monitor-app", "--release", "--bin", bin_name],
            workspace_dir,
            env=env,
        )


def stage_payload(workspace_dir: Path, stage_dir: Path) -> None:
    release_dir = workspace_dir / "target" / "release"
    monitor_path = release_dir / MONITOR_EXE
    host_path = release_dir / HOST_EXE
    if not monitor_path.is_file():
        raise RuntimeError(f"Missing release binary: {monitor_path}")
    if not host_path.is_file():
        raise RuntimeError(f"Missing release binary: {host_path}")

    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    stage_dir.mkdir(parents=True, exist_ok=True)

    shutil.copy2(monitor_path, stage_dir / MONITOR_EXE)
    shutil.copy2(host_path, stage_dir / HOST_EXE)


def main() -> None:
    parser = argparse.ArgumentParser(description="Build CyberMonitor installer")
    parser.add_argument(
        "--skip-build",
        action="store_true",
        help="Skip cargo release build and only package existing binaries",
    )
    args = parser.parse_args()

    script_dir = Path(__file__).resolve().parent
    workspace_dir = repo_root(script_dir)

    config = load_config(script_dir / "make_setup_config.json")
    seven_zip = find_7z(config, script_dir)
    makensis = find_makensis(config, script_dir)
    version = read_workspace_version(workspace_dir)

    if not args.skip_build:
        build_release_binaries(workspace_dir)

    output_dir = workspace_dir / "output" / PRODUCT_KEY / version
    stage_dir = output_dir / "setup_stage"
    archive_path = output_dir / "app" / "app.7z"

    stage_payload(workspace_dir, stage_dir)
    build_archive(seven_zip, stage_dir, archive_path)
    build_nsis(
        makensis,
        script_dir / "make_setup.nsi",
        output_dir,
        version,
        INSTALLER_BASENAME,
    )

    installer = output_dir / f"{INSTALLER_BASENAME}_{version}_Setup.exe"
    print(f"Installer ready: {installer}")


if __name__ == "__main__":
    main()
