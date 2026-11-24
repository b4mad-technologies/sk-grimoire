import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

process.chdir(fileURLToPath(new URL('..', import.meta.url)));

const version = process.argv[2] ?? '0.0.0-ci';
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)
    && version !== '0.0.0-ci' && version !== '0.0.0-bootstrap') {
    throw new Error(`Expected a stable npm version, got ${version}`);
}

const bindgenVersion = '0.2.120';
const lockfile = readFileSync('Cargo.lock', 'utf8');
if (!lockfile.includes(`\nname = "wasm-bindgen"\nversion = "${bindgenVersion}"\n`)) {
    throw new Error('Update the wasm-bindgen CLI pin to match Cargo.lock.');
}
if (execFileSync('wasm-bindgen', ['--version'], { encoding: 'utf8' }).trim()
    !== `wasm-bindgen ${bindgenVersion}`) {
    throw new Error(`Install wasm-bindgen-cli ${bindgenVersion} before building.`);
}

const crates = ['spellkave-validate-wasm', 'spellkave-authoring-wasm'];
execFileSync('cargo', [
    'build', '--locked', '--release', '--target-dir', 'target',
    '--target', 'wasm32-unknown-unknown', ...crates.flatMap(crate => ['-p', crate]),
], {
    stdio: 'inherit',
    env: {
        ...process.env,
        CARGO_PROFILE_RELEASE_STRIP: 'debuginfo',
        CARGO_ENCODED_RUSTFLAGS: [
            `--remap-path-prefix=${process.cwd()}=.`,
            `--remap-path-prefix=${resolve(process.env.CARGO_HOME ?? `${homedir()}/.cargo`)}=/cargo`,
        ].join('\u001f'),
    },
});

const output = resolve('target/wasm-npm', version);
rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });

for (const crate of crates) {
    const name = crate.slice('spellkave-'.length);
    const dir = resolve(output, name);
    const binary = `target/wasm32-unknown-unknown/release/${crate.replaceAll('-', '_')}.wasm`;

    for (const [target, subdir] of [['nodejs', 'node'], ['web', 'browser']]) {
        execFileSync('wasm-bindgen', [
            binary, '--target', target, '--out-dir', `${dir}/${subdir}`, '--out-name', 'index',
        ], { stdio: 'inherit' });
    }

    renameSync(`${dir}/node/index.js`, `${dir}/node/index.cjs`);
    renameSync(`${dir}/node/index.d.ts`, `${dir}/node/index.d.cts`);

    const node = { types: './node/index.d.cts', default: './node/index.cjs' };
    const browser = { types: './browser/index.d.ts', default: './browser/index.js' };
    const manifest = {
        name: `@spellkave/${name}`,
        version,
        description: `Spellkave content language ${name.split('-')[0]} bindings for Node and browsers`,
        license: 'PolyForm-Shield-1.0.0',
        repository: { type: 'git', url: 'git+https://github.com/b4mad-technologies/sk-grimoire.git' },
        type: 'module',
        main: node.default,
        types: node.types,
        browser: browser.default,
        exports: {
            '.': { browser, node, default: browser },
            './node': node,
            './browser': browser,
            './node/wasm': './node/index_bg.wasm',
            './browser/wasm': './browser/index_bg.wasm',
        },
        files: ['node', 'browser', 'LICENSE', 'NOTICE'],
        publishConfig: { access: 'public', registry: 'https://registry.npmjs.org/' },
    };
    writeFileSync(`${dir}/package.json`, `${JSON.stringify(manifest, null, 2)}\n`);
    for (const file of ['LICENSE', 'NOTICE']) {
        copyFileSync(file, `${dir}/${file}`);
    }
    copyFileSync(`crates/${crate}/README.md`, `${dir}/README.md`);
    execFileSync('npm', ['pack', dir, '--pack-destination', output], { stdio: 'inherit' });
}
