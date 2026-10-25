#!/usr/bin/env python3
"""Build a locked, versioned mqtt_dali_v2 archive; no deployment or publication."""
import argparse
import gzip
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def installer_module(root):
    spec = importlib.util.spec_from_file_location('dali_installer', root / 'packaging/install.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(arguments, root, environment=None):
    return subprocess.check_output(arguments, cwd=root, env=environment, text=True).strip()


def source_metadata(root, allow_dirty=False):
    manifest = tomllib.loads((root / 'Cargo.toml').read_text())
    locked = tomllib.loads((root / 'Cargo.lock').read_text())
    toolchain = tomllib.loads((root / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    pins = [manifest[section][name] for section, name in (
        ('dependencies', 'mqtt-bridge-kit'), ('dependencies', 'tracing-init'),
        ('dev-dependencies', 'wait-lint'), ('dev-dependencies', 'mqtt-test-broker'))]
    urls = {entry['git'] for entry in pins}
    revisions = {entry['rev'] for entry in pins}
    if len(urls) != 1 or not next(iter(urls)).startswith('https://') or len(revisions) != 1:
        raise ValueError('All SDK dependencies must use one HTTPS URL and exact revision')
    sdk = next(iter(revisions))
    if len(sdk) != 40 or any(c not in '0123456789abcdef' for c in sdk):
        raise ValueError('SDK revision must be a full commit SHA')
    expected_source = 'git+{}?rev={}#{}'.format(next(iter(urls)), sdk, sdk)
    for name in ('mqtt-bridge-kit', 'tracing-init', 'wait-lint', 'mqtt-test-broker'):
        if not any(package['name'] == name and package.get('source') == expected_source for package in locked['package']):
            raise ValueError('Cargo.lock does not match the SDK pin: ' + name)
    dirty = bool(run(['git', 'status', '--porcelain', '--untracked-files=normal'], root))
    if dirty and not allow_dirty:
        raise ValueError('Release source must be clean; --allow-dirty creates a development artifact only')
    rustc = run(['rustc', '-Vv'], root)
    if 'release: ' + toolchain not in rustc.splitlines():
        raise ValueError('rustc must match pinned toolchain ' + toolchain)
    return {'schema': 1, 'project': 'mqtt_dali_v2', 'version': manifest['package']['version'],
            'source_commit': run(['git', 'rev-parse', 'HEAD'], root),
            'source_date_epoch': int(run(['git', 'show', '-s', '--format=%ct', 'HEAD'], root)),
            'sdk_revision': sdk, 'rustc': rustc, 'dirty': dirty,
            'cargo_lock_sha256': installer_module(root).digest(root / 'Cargo.lock')}


def assemble(root, binaries, output, metadata, notices=""):
    """Assemble verified build outputs. Separated from cargo for package tests."""
    check = installer_module(root)
    check.expected_kind(metadata['target'])
    release_id = '{}-{}-{}'.format(metadata['version'], metadata['source_commit'][:12], metadata['target'])
    if metadata['dirty']:
        release_id += '-dirty'
    if not check.safe_name(release_id):
        raise ValueError('Invalid release filename')
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    basename = 'mqtt_dali_v2-' + release_id
    archive = output / (basename + '.tar.gz')
    if archive.exists() or Path(str(archive) + '.sha256').exists():
        raise ValueError('Release output already exists; choose an empty output directory')
    with tempfile.TemporaryDirectory(prefix='dali-package-') as folder:
        package = Path(folder) / basename
        package.mkdir()
        for name in ('mqtt_dali_v2', 'dali_hat_emulator'):
            target = package / 'bin' / name
            target.parent.mkdir(exist_ok=True)
            shutil.copyfile(Path(binaries) / name, target)
            target.chmod(0o755)
        for source, target, mode in (
            ('packaging/install.py', 'install.py', 0o755),
            ('packaging/start', 'libexec/start', 0o755),
            ('packaging/mqtt_dali_v2.service', 'packaging/mqtt_dali_v2.service', 0o644),
            ('packaging/bridge.env.example', 'packaging/bridge.env.example', 0o644),
            ('packaging/logging.toml', 'packaging/logging.toml', 0o644),
            ('README.md', 'README.md', 0o644),
            ('scripts/release_smoke.py', 'scripts/release_smoke.py', 0o755),
            ('packaging/licenses/preact.txt', 'licenses/preact.txt', 0o644),
        ):
            path = package / target
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(root / source, path)
            path.chmod(mode)
        for directory in ('docs', 'examples/emulator'):
            for source in sorted((root / directory).rglob('*')):
                if source.is_file() and source.suffix in ('.md', '.json', '.toml') and '.ledger.json' not in source.name:
                    target = package / source.relative_to(root)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, target)
                    target.chmod(0o644)
        if (root / 'LICENSE').is_file():
            shutil.copyfile(root / 'LICENSE', package / 'LICENSE')
            (package / 'LICENSE').chmod(0o644)
        (package / 'THIRD_PARTY_NOTICES.txt').write_text(notices)
        (package / 'THIRD_PARTY_NOTICES.txt').chmod(0o644)
        files = {path.relative_to(package).as_posix(): {'sha256': check.digest(path), 'mode': stat_mode(path)}
                 for path in sorted(package.rglob('*')) if path.is_file()}
        manifest = dict(metadata, release_id=release_id, files=files)
        (package / 'release.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
        hashes = {name: record['sha256'] for name, record in files.items()}
        hashes['release.json'] = check.digest(package / 'release.json')
        (package / 'SHA256SUMS').write_text(''.join('{}  {}\n'.format(hashes[name], name) for name in sorted(hashes)))
        check.verify_package(package)
        descriptor, temporary_name = tempfile.mkstemp(dir=output, prefix='.release-')
        try:
            with os.fdopen(descriptor, 'wb') as stream:
                with gzip.GzipFile(filename='', mode='wb', fileobj=stream, mtime=metadata['source_date_epoch']) as compressed:
                    with tarfile.open(fileobj=compressed, mode='w', format=tarfile.PAX_FORMAT) as tar:
                        for path in (package, *sorted(package.rglob('*'))):
                            info = tar.gettarinfo(str(path), arcname=basename + (('/' + path.relative_to(package).as_posix()) if path != package else ''))
                            info.uid = info.gid = 0
                            info.uname = info.gname = 'root'
                            info.mtime = metadata['source_date_epoch']
                            info.pax_headers = {}
                            if path.is_file():
                                with path.open('rb') as source:
                                    tar.addfile(info, source)
                            else:
                                info.mode = 0o755
                                tar.addfile(info)
            Path(temporary_name).chmod(0o644)
            os.link(temporary_name, archive)
        finally:
            Path(temporary_name).unlink(missing_ok=True)
    checksum = check.digest(archive)
    Path(str(archive) + '.sha256').write_text('{}  {}\n'.format(checksum, archive.name))
    return {'archive': str(archive), 'sha256': checksum, 'release_id': release_id}


def dependency_notices(info):
    """Include dependency declarations and supplied license/notice texts.

    Cargo metadata also lists build/test dependencies; the notice set is deliberately
    inclusive. Preact's embedded-browser license is a separate packaged file.
    """
    result = ['Third-party Rust dependencies (including build/test tools).',
              'Embedded Preact: see licenses/preact.txt.\n']
    for package in sorted(info['packages'], key=lambda p: (p['name'], p['version'])):
        if package.get('source') is None:
            continue
        directory = Path(package['manifest_path']).parent
        result += ['=' * 72, package['name'] + ' ' + package['version'],
                   'License: ' + (package.get('license') or 'see supplied license file'),
                   'Source: ' + package['source']]
        candidates = {path for path in directory.iterdir() if path.is_file()
                      and path.name.lower().startswith(('license', 'copying', 'copyright', 'notice'))}
        if package.get('license_file'):
            candidates.add(directory / package['license_file'])
        # Workspace Git crates may inherit the repository's license text.
        if not candidates and package['source'].startswith('git+'):
            for parent in directory.parents:
                candidates.update(path for path in parent.glob('LICENSE*') if path.is_file())
                if candidates or (parent / '.git').exists():
                    break
        for path in sorted(candidates):
            result += ['--- ' + path.name + ' ---', path.read_text(errors='replace')]
        result.append('')
    return '\n'.join(result) + '\n'


def stat_mode(path):
    return path.stat().st_mode & 0o777


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default='armv7-unknown-linux-musleabihf')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist')
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--allow-dirty', action='store_true', help='Development artifact only; recorded in release.json')
    args = parser.parse_args()
    try:
        metadata = source_metadata(ROOT, args.allow_dirty)
        metadata['target'] = args.target
        installer_module(ROOT).expected_kind(args.target)
        environment = dict(os.environ, SOURCE_DATE_EPOCH=str(metadata['source_date_epoch']))
        environment.pop('BUILT_OVERRIDE_BUILT_TIME_UTC', None)
        command = ['cargo', 'build', '--locked', '--release', '--target', args.target,
                   '--bin', 'mqtt_dali_v2', '--bin', 'dali_hat_emulator']
        if args.offline:
            command.append('--offline')
        subprocess.run(command, cwd=ROOT, env=environment, check=True)
        metadata_command = ['cargo', 'metadata', '--locked', '--filter-platform', args.target, '--format-version', '1']
        if args.offline:
            metadata_command.append('--offline')
        info = json.loads(run(metadata_command, ROOT, environment))
        for name in ('mqtt-bridge-kit', 'tracing-init'):
            if not any(package['name'] == name and package.get('source', '').endswith('#' + metadata['sdk_revision']) for package in info['packages']):
                raise ValueError('Resolved SDK does not match the declared Git revision: ' + name)
        binaries = Path(info['target_directory']) / args.target / 'release'
        print(json.dumps(assemble(ROOT, binaries, args.output, metadata, dependency_notices(info)), sort_keys=True))
    except (ValueError, OSError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        parser.exit(1, 'Release build failed: {}\n'.format(error))


if __name__ == '__main__':
    main()
