// Runs the Tauri CLI with llvm-mingw on PATH when it is installed (needed for `windres`
// on the GNU toolchain, see src-tauri/.cargo/config.toml.example). No effect on MSVC setups.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const env = { ...process.env };
const extra = path.join(homedir(), 'tools', 'llvm-mingw', 'bin');
if (existsSync(extra)) {
  // Windows env keys are case-insensitive but a spread object is not: reuse the real key.
  const key = Object.keys(env).find((k) => k.toUpperCase() === 'PATH') ?? 'PATH';
  env[key] = `${env[key] ?? ''}${path.delimiter}${extra}`;
}
const cli = path.join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const result = spawnSync(process.execPath, [cli, ...process.argv.slice(2)], { stdio: 'inherit', env });
process.exit(result.status ?? 1);
