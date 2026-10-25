"""P9 release/installer and service-launcher contract. No host installation."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


installer = load('p9_install', ROOT / 'packaging/install.py')
release = load('p9_release', ROOT / 'scripts/release.py')


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='dali-release-test-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.source = self.base / 'source'
        self.source.mkdir()
        shutil.copytree(ROOT / 'packaging', self.source / 'packaging')
        (self.source / 'README.md').write_text('Package test fixture\n')
        (self.source / 'scripts').mkdir()
        shutil.copyfile(ROOT / 'scripts/release_smoke.py', self.source / 'scripts/release_smoke.py')
        self.binaries = self.base / 'build'
        self.binaries.mkdir()
        # ELF32 little-endian ARM header: enough for the structural verifier.
        header = bytearray(32)
        header[:6] = b'\x7fELF\x01\x01'
        header[18:20] = (40).to_bytes(2, 'little')
        for name in ('mqtt_dali_v2', 'dali_hat_emulator'):
            (self.binaries / name).write_bytes(header + name.encode())
        self.metadata = dict(schema=1, project='mqtt_dali_v2', version='2.0.0-dev.0',
                             source_commit='a'*40, source_date_epoch=1000,
                             sdk_revision='b'*40, rustc='fixture compiler',
                             dirty=False, target='armv7-unknown-linux-musleabihf')
        self.number = 0

    def package(self, commit=None):
        self.number += 1
        metadata = dict(self.metadata)
        if commit:
            metadata['source_commit'] = commit
        output = self.base / ('output' + str(self.number))
        artifact = release.assemble(self.source, self.binaries, output, metadata)
        extracted = self.base / ('extracted' + str(self.number))
        with tarfile.open(artifact['archive']) as archive:
            archive.extractall(extracted, filter='data')
        return next(extracted.iterdir()), artifact

    def rehash(self, package):
        data = json.loads((package / 'release.json').read_text())
        for name, record in data['files'].items():
            record['sha256'] = installer.digest(package / name)
        (package / 'release.json').write_text(json.dumps(data, sort_keys=True))
        hashes = {name: value['sha256'] for name, value in data['files'].items()}
        hashes['release.json'] = installer.digest(package / 'release.json')
        (package / 'SHA256SUMS').write_text(''.join('{}  {}\n'.format(hashes[name], name) for name in sorted(hashes)))

    def test_sdk_sources_are_https_and_timestamp_input_invalidates_build(self):
        cargo = tomllib.loads((ROOT / 'Cargo.toml').read_text())
        for section, name in [('dependencies', 'mqtt-bridge-kit'), ('dependencies', 'tracing-init'),
                              ('dev-dependencies', 'wait-lint'), ('dev-dependencies', 'mqtt-test-broker')]:
            self.assertTrue(cargo[section][name]['git'].startswith('https://'), 'SDK source must be fetchable over HTTPS')
        self.assertIn('cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH', (ROOT / 'build.rs').read_text())

    def test_build_metadata_refuses_dirty_source_and_mismatched_sdk(self):
        for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'):
            shutil.copyfile(ROOT / name, self.source / name)
        dirty = False
        compiler = "1.99.0"

        def run(arguments, root, environment=None):
            if arguments[:2] == ['git', 'status']:
                return ' M README.md' if dirty else ''
            if arguments[:2] == ['git', 'rev-parse']:
                return 'a'*40
            if arguments[:2] == ['git', 'show']:
                return '1000'
            if arguments == ['rustc', '-Vv']:
                return 'rustc ' + compiler + '\nrelease: ' + compiler
            self.fail(str(arguments))

        with mock.patch.object(release, 'run', side_effect=run):
            data = release.source_metadata(self.source)
            self.assertEqual(data['source_commit'], 'a'*40)
            self.assertEqual(data['source_date_epoch'], 1000)
            self.assertFalse(data['dirty'])
            compiler = '1.98.0'
            with self.assertRaisesRegex(ValueError, 'must match pinned toolchain'):
                release.source_metadata(self.source)
            compiler = '1.99.0'
            dirty = True
            with self.assertRaisesRegex(ValueError, 'must be clean'):
                release.source_metadata(self.source)
            self.assertTrue(release.source_metadata(self.source, allow_dirty=True)['dirty'])
            lock = self.source / 'Cargo.lock'
            lock.write_text(lock.read_text().replace(data['sdk_revision'], 'c'*40))
            with self.assertRaisesRegex(ValueError, 'does not match the SDK pin'):
                release.source_metadata(self.source, allow_dirty=True)

    def test_dependency_notice_texts_are_included(self):
        dependency = self.base / 'dependency'
        dependency.mkdir()
        (dependency / 'LICENSE').write_text('Fixture copyright and permission\n')
        data = {'packages': [dict(name='example', version='1.0', source='registry+example',
                                 license='MIT', manifest_path=str(dependency / 'Cargo.toml'))]}
        notices = release.dependency_notices(data)
        self.assertIn('example 1.0', notices)
        self.assertIn('Fixture copyright and permission', notices)

    def test_executable_mode_damage_is_detected(self):
        package, _ = self.package()
        (package / 'bin/mqtt_dali_v2').chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'checksum or mode mismatch'):
            installer.install(package, self.base / 'root')
        self.assertFalse((self.base / 'root').exists())

    def test_archive_is_deterministic_and_contains_no_legacy_binary(self):
        first, a = self.package()
        second, b = self.package()
        self.assertEqual(a['sha256'], b['sha256'])
        self.assertEqual(installer.verify_package(first)['release_id'], installer.verify_package(second)['release_id'])
        self.assertEqual(sorted(p.name for p in (first / 'bin').iterdir()), ['dali_hat_emulator', 'mqtt_dali_v2'])
        with self.assertRaisesRegex(ValueError, 'already exists'):
            release.assemble(self.source, self.binaries, Path(a['archive']).parent, self.metadata)

    def test_staged_install_upgrade_rollback_preserve_configuration_and_v1(self):
        package, _ = self.package()
        root = self.base / 'root'
        legacy = root / 'usr/local/bin/mqtt_dali'
        legacy.parent.mkdir(parents=True)
        legacy.write_bytes(b'legacy executable')
        with mock.patch.object(installer.subprocess, 'run', side_effect=AssertionError('Host command during staged install')):
            first = installer.install(package, root)
            config = root / 'etc/mqtt_dali_v2/bridge.env'
            self.assertEqual(config.stat().st_mode & 0o777, 0o600)
            config.write_text('private custom configuration\n')
            installer.install(package, root)
            upgraded, _ = self.package('c'*40)
            new = installer.install(upgraded, root)
            self.assertNotEqual(first['release_id'], new['release_id'])
            self.assertEqual(os.readlink(root / 'opt/mqtt_dali_v2/current'), '/opt/mqtt_dali_v2/releases/' + new['release_id'])
            installer.install(package, root)
        self.assertEqual(config.read_text(), 'private custom configuration\n')
        self.assertEqual(legacy.read_bytes(), b'legacy executable')
        self.assertEqual(os.readlink(root / 'opt/mqtt_dali_v2/current'), '/opt/mqtt_dali_v2/releases/' + first['release_id'])
        unit = (root / 'etc/systemd/system/mqtt_dali_v2.service').read_text()
        self.assertIn('ExecStart=/opt/mqtt_dali_v2/current/libexec/start', unit)
        self.assertNotIn(str(root), unit)

    def test_linux_install_only_creates_account_and_reloads_units(self):
        import contextlib
        import grp
        import pwd
        from types import SimpleNamespace
        package, _ = self.package()
        account = SimpleNamespace(pw_uid=1200, pw_gid=1200)
        original_destination = installer.destination
        for missing in (False, True):
            root = (self.base / ('linux-missing' if missing else 'linux-existing')).resolve()
            with contextlib.ExitStack() as mocks:
                mocks.enter_context(mock.patch.object(installer.platform, 'system', return_value='Linux'))
                mocks.enter_context(mock.patch.object(installer.platform, 'machine', return_value='armv7l'))
                mocks.enter_context(mock.patch.object(installer.os, 'geteuid', return_value=0))
                mocks.enter_context(mock.patch.object(installer, 'destination', side_effect=lambda _, name: original_destination(root, name)))
                mocks.enter_context(mock.patch.object(installer.shutil, 'which', side_effect=lambda tool: '/mock/tool' if missing or tool == 'systemctl' else None))
                mocks.enter_context(mock.patch.object(grp, 'getgrnam', return_value=SimpleNamespace(gr_gid=1200)))
                mocks.enter_context(mock.patch.object(pwd, 'getpwnam', side_effect=[KeyError('absent'), account] if missing else None, return_value=account))
                chown = mocks.enter_context(mock.patch.object(installer.os, 'chown'))
                commands = mocks.enter_context(mock.patch.object(installer.subprocess, 'run'))
                installer.install(package, '/')
                called = [call.args[0] for call in commands.call_args_list]
                self.assertEqual(called[-1], ['systemctl', 'daemon-reload'])
                self.assertEqual([command[0] for command in called], ['useradd', 'systemctl'] if missing else ['systemctl'])
                self.assertFalse(any('start' in command or 'enable' in command or 'restart' in command for command in called))
                chown.assert_called_once_with(root / 'var/lib/mqtt_dali_v2', 1200, 1200)

    def test_existing_service_account_without_its_group_is_refused_before_changes(self):
        import grp
        import pwd
        from types import SimpleNamespace
        package, _ = self.package()
        root = (self.base / 'linux').resolve()
        original_destination = installer.destination
        with mock.patch.object(installer.platform, 'system', return_value='Linux'), \
             mock.patch.object(installer.platform, 'machine', return_value='armv7l'), \
             mock.patch.object(installer.os, 'geteuid', return_value=0), \
             mock.patch.object(installer, 'destination', side_effect=lambda _, name: original_destination(root, name)), \
             mock.patch.object(installer.shutil, 'which', return_value='/mock/tool'), \
             mock.patch.object(grp, 'getgrnam', side_effect=[SimpleNamespace(gr_gid=20), KeyError('group absent')]), \
             mock.patch.object(pwd, 'getpwnam', return_value=SimpleNamespace(pw_uid=1200, pw_gid=1200)), \
             mock.patch.object(installer.subprocess, 'run') as commands:
            with self.assertRaisesRegex(ValueError, 'matching non-root primary group'):
                installer.install(package, '/')
            commands.assert_not_called()
        self.assertFalse(root.exists())


    def test_smoke_refuses_python_optimization_before_starting_processes(self):
        import sys
        result = subprocess.run([sys.executable, '-O', str(ROOT / 'scripts/release_smoke.py'), str(self.source)],
                                env={'PATH': '/nonexistent'}, capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 2)
        self.assertIn('Python optimization disables acceptance assertions', result.stderr)

    def test_failed_systemd_reload_keeps_previous_binary_selected(self):
        import grp
        import pwd
        from types import SimpleNamespace
        package, _ = self.package()
        root = (self.base / 'linux-reload').resolve()
        installer.install(package, root)
        current = root / 'opt/mqtt_dali_v2/current'
        original = os.readlink(current)
        updated, _ = self.package('c'*40)
        original_destination = installer.destination
        with mock.patch.object(installer.platform, 'system', return_value='Linux'), \
             mock.patch.object(installer.platform, 'machine', return_value='armv7l'), \
             mock.patch.object(installer.os, 'geteuid', return_value=0), \
             mock.patch.object(installer.os, 'chown'), \
             mock.patch.object(installer, 'destination', side_effect=lambda _, name: original_destination(root, name)), \
             mock.patch.object(installer.shutil, 'which', return_value='/mock/tool'), \
             mock.patch.object(grp, 'getgrnam', return_value=SimpleNamespace(gr_gid=1200)), \
             mock.patch.object(pwd, 'getpwnam', return_value=SimpleNamespace(pw_uid=1200, pw_gid=1200)), \
             mock.patch.object(installer.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, ['systemctl', 'daemon-reload'])):
            with self.assertRaises(subprocess.CalledProcessError):
                installer.install(updated, '/')
        self.assertEqual(os.readlink(current), original, 'failed reload selected a new binary')


    def test_private_umask_still_leaves_release_readable_by_service_account(self):
        package, _ = self.package()
        old = os.umask(0o077)
        try:
            result = installer.install(package, self.base / 'root')
        finally:
            os.umask(old)
        root = self.base / 'root'
        target = root / 'opt/mqtt_dali_v2/releases' / result['release_id']
        for directory in (target.parent.parent, target.parent, target, target / 'bin', target / 'libexec'):
            self.assertEqual(directory.stat().st_mode & 0o777, 0o755, str(directory))

    def test_corrupt_payload_refused_before_destination_writes(self):
        package, _ = self.package()
        (package / 'README.md').write_bytes(b'tampered')
        root = self.base / 'root'
        with self.assertRaisesRegex(ValueError, 'checksum'):
            installer.install(package, root)
        self.assertFalse(root.exists())

    def test_staging_symlink_escape_refused_without_external_writes(self):
        package, _ = self.package()
        root = self.base / 'root'
        root.mkdir()
        outside = self.base / 'outside'
        outside.mkdir()
        (outside / 'sentinel').write_bytes(b'untouched')
        (root / 'etc').symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'escapes installation root'):
            installer.install(package, root)
        self.assertEqual([p.name for p in outside.iterdir()], ['sentinel'])
        self.assertFalse((root / 'opt').exists())

    def test_unrelated_destination_conflicts_precede_all_changes(self):
        package, _ = self.package()
        for name in ('usr/local/bin/mqtt_dali_v2', 'etc/systemd/system/mqtt_dali_v2.service',
                     'opt/mqtt_dali_v2/current'):
            with self.subTest(name=name):
                root = self.base / ('root' + str(len(name)))
                conflict = root / name
                conflict.parent.mkdir(parents=True)
                conflict.write_bytes(b'unrelated content')
                before = sorted(str(p.relative_to(root)) for p in root.rglob('*'))
                with self.assertRaisesRegex(ValueError, 'unrelated'):
                    installer.install(package, root)
                self.assertEqual(conflict.read_bytes(), b'unrelated content')
                self.assertEqual(before, sorted(str(p.relative_to(root)) for p in root.rglob('*')))

    def test_wrong_architecture_and_unsafe_members_are_refused(self):
        package, _ = self.package()
        metadata = json.loads((package / 'release.json').read_text())
        metadata['target'] = 'aarch64-unknown-linux-musl'
        (package / 'release.json').write_text(json.dumps(metadata))
        self.rehash(package)
        with self.assertRaisesRegex(ValueError, 'does not match release target'):
            installer.verify_package(package)
        for name in ('../escape', '/absolute', 'a/../escape', 'a//b'):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'Invalid package path'):
                installer.relative(name)
        (package / 'unlisted').symlink_to(self.base / 'outside')
        with self.assertRaisesRegex(ValueError, 'regular files'):
            installer.verify_package(package)

    def test_same_release_id_cannot_replace_different_contents(self):
        package, _ = self.package()
        root = self.base / 'root'
        installer.install(package, root)
        before = os.readlink(root / 'opt/mqtt_dali_v2/current')
        (self.source / 'README.md').write_text('different but valid contents')
        conflict, _ = self.package()
        with self.assertRaisesRegex(ValueError, 'Conflicting contents'):
            installer.install(conflict, root)
        self.assertEqual(os.readlink(root / 'opt/mqtt_dali_v2/current'), before)

    def test_interrupted_copy_keeps_current_release_and_config(self):
        package, _ = self.package()
        root = self.base / 'root'
        installer.install(package, root)
        current = root / 'opt/mqtt_dali_v2/current'
        before = os.readlink(current)
        config = root / 'etc/mqtt_dali_v2/bridge.env'
        saved = config.read_bytes()
        upgrade, _ = self.package('c'*40)
        with mock.patch.object(installer.shutil, 'copyfile', side_effect=OSError('Injected copy failure')):
            with self.assertRaisesRegex(OSError, 'Injected copy failure'):
                installer.install(upgrade, root)
        self.assertEqual(os.readlink(current), before)
        self.assertEqual(config.read_bytes(), saved)
        self.assertFalse(list((root / 'opt/mqtt_dali_v2/releases').glob('.install-*')))


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='dali-launcher-test-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        (self.base / 'libexec').mkdir()
        (self.base / 'bin').mkdir()
        shutil.copyfile(ROOT / 'packaging/start', self.base / 'libexec/start')
        binary = self.base / 'bin/mqtt_dali_v2'
        binary.write_text('#!/usr/bin/env python3\nimport json,os,sys\nfrom pathlib import Path\nPath(os.environ["CAPTURE"]).write_text(json.dumps({"argv":sys.argv[1:],"password":os.environ.get("MQTT_DALI_WEB_PASSWORD")}))\n')
        binary.chmod(0o755)
        self.env = {'PATH': os.environ['PATH'], 'CAPTURE': str(self.base / 'capture.json'),
                    'MQTT_DALI_INSTANCE': 'Kitchen $(touch sentinel)', 'MQTT_DALI_BROKER': '127.0.0.1:1883'}

    def launch(self):
        return subprocess.run(['sh', str(self.base / 'libexec/start')], env=self.env,
                              cwd=self.base, capture_output=True, text=True, timeout=5)

    def test_literals_uart_and_disabled_web(self):
        result = self.launch()
        self.assertEqual(result.returncode, 0, result.stderr)
        capture = json.loads((self.base / 'capture.json').read_text())
        self.assertEqual(capture['argv'][1], self.env['MQTT_DALI_INSTANCE'])
        self.assertIn('--uart', capture['argv'])
        self.assertNotIn('--web-bind', capture['argv'])
        self.assertFalse((self.base / 'sentinel').exists())

    def test_tcp_omits_uart_and_password_stays_out_of_argv(self):
        self.env.update(MQTT_DALI_HAT_TCP='127.0.0.1:5580', MQTT_DALI_WEB_BIND='127.0.0.1:8080',
                        MQTT_DALI_WEB_ORIGIN='http://127.0.0.1:8080', MQTT_DALI_WEB_PASSWORD='private ; $(touch sentinel)')
        result = self.launch()
        self.assertEqual(result.returncode, 0, result.stderr)
        capture = json.loads((self.base / 'capture.json').read_text())
        self.assertIn('--hat-tcp', capture['argv'])
        self.assertNotIn('--uart', capture['argv'])
        self.assertIn('--web-bind', capture['argv'])
        self.assertNotIn('--web-password', capture['argv'])
        self.assertNotIn(self.env['MQTT_DALI_WEB_PASSWORD'], capture['argv'])
        self.assertEqual(capture['password'], self.env['MQTT_DALI_WEB_PASSWORD'])

    def test_web_requires_explicit_credentials_before_exec(self):
        self.env['MQTT_DALI_WEB_BIND'] = '127.0.0.1:8080'
        for origin in ('', 'http://127.0.0.1:8080'):
            self.env['MQTT_DALI_WEB_ORIGIN'] = origin
            result = self.launch()
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((self.base / 'capture.json').exists())


if __name__ == '__main__':
    unittest.main()
