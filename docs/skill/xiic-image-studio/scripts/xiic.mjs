#!/usr/bin/env node
// xiic-image-studio CLI 的薄封装。
//
// 职责只有三件：定位二进制、转发参数、归一化退出码/输出流。
// 生图逻辑全在 Rust 二进制里，这里不重写任何业务规则。
// 只用 Node 内置模块，无第三方依赖。
//
// 用 .mjs 后缀 + ESM 语法：这样无论 skill 装在仓库内（package.json 里
// "type": "module"）还是独立目录，行为都一致。

import { existsSync, statSync } from 'node:fs';
import { join, resolve, delimiter } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// scripts/ -> skill 根目录 -> （若 skill 仍在仓库内）仓库根目录
const SKILL_DIR = resolve(fileURLToPath(new URL('.', import.meta.url)), '..');
const REPO_ROOT = resolve(SKILL_DIR, '..', '..', '..');

const BIN_NAME = 'xiic-image-studio';

function candidates() {
  const list = [];

  // 1. 环境变量显式指定（最高优先级）
  const env = process.env.XIIC_IMAGE_STUDIO_BIN;
  if (env) list.push(env);

  // 2. PATH
  for (const dir of (process.env.PATH || '').split(delimiter)) {
    if (dir) list.push(join(dir, BIN_NAME));
  }

  // 3. 仓库内的 release 构建
  list.push(join(REPO_ROOT, 'src-tauri', 'target', 'release', BIN_NAME));

  // 4. macOS 安装包
  list.push(join('/Applications', `${BIN_NAME}.app`, 'Contents', 'MacOS', BIN_NAME));

  return list;
}

function isExecutable(path) {
  try {
    return existsSync(path) && statSync(path).isFile();
  } catch {
    return false;
  }
}

function resolveBinary() {
  for (const candidate of candidates()) {
    if (isExecutable(candidate)) return candidate;
  }
  return null;
}

function notFoundMessage() {
  return [
    `找不到 ${BIN_NAME} 可执行文件。已尝试：`,
    ...candidates().map((c) => `  - ${c}`),
    '',
    `用环境变量指定：export XIIC_IMAGE_STUDIO_BIN=/path/to/${BIN_NAME}`,
  ].join('\n');
}

function fail(message, code) {
  process.stderr.write(message.endsWith('\n') ? message : message + '\n');
  process.exit(code);
}

function main() {
  const argv = process.argv.slice(2);

  // --which：只打印解析到的二进制路径，供排查用
  if (argv.includes('--which')) {
    const bin = resolveBinary();
    if (!bin) fail(notFoundMessage(), 127);
    process.stdout.write(bin + '\n');
    return;
  }

  if (argv.length === 0) {
    fail('缺少子命令。用法：xiic.mjs generate|providers|--help|--which', 2);
  }

  const bin = resolveBinary();
  if (!bin) fail(notFoundMessage(), 127);

  const result = spawnSync(bin, argv, { encoding: 'utf8' });

  if (result.error) {
    fail(`调用 ${bin} 失败：${result.error.message}`, 1);
  }

  const stdout = result.stdout || '';
  const stderr = result.stderr || '';

  // 归一化：--json 时 stdout 必须是合法 JSON，否则明确报出来，
  // 免得脚本消费方把半截输出当成结果。
  if (argv.includes('--json') && stdout.trim()) {
    try {
      JSON.parse(stdout);
    } catch (e) {
      if (stdout) process.stdout.write(stdout);
      process.stderr.write(`\n[xiic] --json 输出不是合法 JSON：${e.message}\n`);
      process.exit(result.status === null ? 1 : result.status);
    }
  }

  if (stdout) process.stdout.write(stdout);
  if (stderr) {
    process.stderr.write(stderr);
    // 旧二进制（CLI 之前那版）不认识子命令，会把它当 GUI 参数直接启动。
    // 这里点破它，免得用户以为是自己命令写错了。
    if (stderr.includes('未知子命令')) {
      process.stderr.write(
        `[xiic] 提示：找到的 ${BIN_NAME} 不支持 CLI 子命令（版本过旧）。\n` +
          `[xiic] 请重新构建：cd src-tauri && cargo build --release\n`
      );
    }
  }

  process.exit(result.status === null ? 1 : result.status);
}

main();
