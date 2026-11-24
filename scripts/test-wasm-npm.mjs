import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const version = process.argv[2] ?? '0.0.0-ci';
const packages = ['@spellkave/validate-wasm', '@spellkave/authoring-wasm'];
const sample = readFileSync(join(root, 'crates/spellkave-format/tests/fixtures/valid_manifest.toml'), 'utf8');
const project = mkdtempSync(join(realpathSync(tmpdir()), 'grimoire-wasm-npm-'));

try {
    writeFileSync(join(project, 'package.json'), JSON.stringify({ private: true }));
    const tarballs = packages.map(name => resolve(
        root, 'target/wasm-npm', version, `${name.replace('@', '').replace('/', '-')}-${version}.tgz`,
    ));
    execFileSync('npm', ['install', '--ignore-scripts', '--no-audit', '--no-fund', ...tarballs], {
        cwd: project, stdio: 'inherit',
    });

    const require = createRequire(join(project, 'package.json'));
    const [validator, authoring] = packages.map(name => {
        assert.equal(require.resolve(name), require.resolve(`${name}/node`));
        for (const target of ['node', 'browser']) {
            const wasm = readFileSync(require.resolve(`${name}/${target}/wasm`));
            assert.ok(WebAssembly.validate(wasm), `${name} ${target} Wasm is invalid`);
        }
        return require(name);
    });

    assert.deepEqual(JSON.parse(validator.validate_manifest_toml(sample)), []);
    assert.deepEqual(JSON.parse(authoring.applyset_root_info_json(sample)), {
        applyset: null, includes: [], bundles: [],
    });
    execFileSync(process.execPath, ['--input-type=module', '-e', `
        import assert from 'node:assert/strict';
        import { validate_manifest_toml } from '@spellkave/validate-wasm';
        import { applyset_root_info_json } from '@spellkave/authoring-wasm';
        assert.equal(typeof validate_manifest_toml, 'function');
        assert.equal(typeof applyset_root_info_json, 'function');
    `], { cwd: project, stdio: 'inherit' });
    console.log('Node smoke test passed');

    const imports = JSON.parse(execFileSync(process.execPath, [
        '--conditions=browser', '--input-type=module', '-e', `
            import assert from 'node:assert/strict';
            import { fileURLToPath } from 'node:url';
            const packages = ${JSON.stringify(packages)};
            console.log(JSON.stringify(Object.fromEntries(packages.map(name => {
                const entry = import.meta.resolve(name);
                assert.equal(entry, import.meta.resolve(name + '/browser'));
                return [name, fileURLToPath(entry)];
            }))));
        `,
    ], { cwd: project, encoding: 'utf8' }));
    const routes = new Map();
    for (const name of packages) {
        const entry = imports[name];
        const binary = require.resolve(`${name}/browser/wasm`);
        imports[name] = '/' + relative(project, entry);
        routes.set(imports[name], ['text/javascript', readFileSync(entry)]);
        routes.set('/' + relative(project, binary), ['application/wasm', readFileSync(binary)]);
    }
    routes.set('/', ['text/html', `<!doctype html>
        <meta charset="utf-8">
        <script type="importmap">${JSON.stringify({ imports })}</script>
        <script type="module">
            try {
                const validator = await import('@spellkave/validate-wasm');
                const authoring = await import('@spellkave/authoring-wasm');
                await Promise.all([validator.default(), authoring.default()]);
                const sample = ${JSON.stringify(sample)};
                if (validator.validate_manifest_toml(sample) !== '[]') {
                    throw new Error('Expected no manifest diagnostics');
                }
                const info = JSON.parse(authoring.applyset_root_info_json(sample));
                if (info.applyset !== null || info.includes.length || info.bundles.length) {
                    throw new Error('Unexpected applyset info');
                }
                await fetch('/result', { method: 'POST', body: 'ok' });
            } catch (error) {
                await fetch('/result', { method: 'POST', body: String(error) });
            }
        </script>`]);

    let chrome;
    const server = createServer();
    try {
        await new Promise((pass, fail) => {
            server.on('error', fail);
            server.on('request', async (request, response) => {
                if (request.method === 'POST' && request.url === '/result') {
                    let result = '';
                    for await (const chunk of request) result += chunk;
                    response.end();
                    if (result === 'ok') pass();
                    else fail(new Error(result));
                    return;
                }
                const route = routes.get(request.url);
                if (!route) {
                    response.writeHead(404).end();
                    return;
                }
                response.writeHead(200, { 'Content-Type': route[0] }).end(route[1]);
            });
            server.listen(0, '127.0.0.1', () => {
                chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
                    '--headless', '--no-first-run', '--no-default-browser-check',
                    `--user-data-dir=${join(project, 'chrome')}`,
                    `http://127.0.0.1:${server.address().port}/`,
                ], { stdio: ['ignore', 'ignore', 'pipe'], timeout: 30_000 });
                let stderr = '';
                chrome.stderr.setEncoding('utf8').on('data', chunk => { stderr += chunk; });
                chrome.on('error', fail);
                chrome.on('exit', (code, signal) => {
                    fail(new Error(`Chrome exited before the browser smoke test finished (${code ?? signal})\n${stderr}`));
                });
            });
        });
        console.log('Chrome smoke test passed');
    } finally {
        if (chrome?.pid && chrome.exitCode === null && chrome.signalCode === null) {
            const stopped = new Promise(done => chrome.once('exit', done));
            chrome.kill();
            await stopped;
        }
        server.close();
    }
} finally {
    rmSync(project, { recursive: true, force: true });
}
