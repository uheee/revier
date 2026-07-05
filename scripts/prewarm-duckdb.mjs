#!/usr/bin/env node
import { copyFileSync, createWriteStream, existsSync, mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { Console } from 'node:console';
import process from 'node:process';
import { setTimeout as delay } from 'node:timers/promises';
import { URL, fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import http from 'node:http';
import https from 'node:https';

const logger = new Console({ stdout: process.stdout, stderr: process.stderr });
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const cargoLockPath = join(repositoryRoot, 'Cargo.lock');
const targets = process.argv.slice(2);
const downloadedArchives = new Map();

const archives = new Map([
  ['x86_64-unknown-linux-gnu', {
    archiveName: 'libduckdb-linux-amd64.zip',
    requiredFiles: ['libduckdb.so', 'duckdb.h'],
  }],
  ['aarch64-unknown-linux-gnu', {
    archiveName: 'libduckdb-linux-arm64.zip',
    requiredFiles: ['libduckdb.so', 'duckdb.h'],
  }],
  ['x86_64-pc-windows-msvc', {
    archiveName: 'libduckdb-windows-amd64.zip',
    requiredFiles: ['duckdb.dll', 'duckdb.lib', 'duckdb.h'],
  }],
  ['aarch64-pc-windows-msvc', {
    archiveName: 'libduckdb-windows-arm64.zip',
    requiredFiles: ['duckdb.dll', 'duckdb.lib', 'duckdb.h'],
  }],
]);

function archiveForTarget(target) {
  if (target.endsWith('apple-darwin')) {
    return {
      archiveName: 'libduckdb-osx-universal.zip',
      requiredFiles: ['libduckdb.dylib', 'duckdb.h'],
    };
  }

  return archives.get(target);
}

function cargoTargetDirectory() {
  const configured = process.env.CARGO_TARGET_DIR;
  if (!configured) {
    return join(repositoryRoot, 'target');
  }

  return isAbsolute(configured) ? configured : join(repositoryRoot, configured);
}

function readLibduckdbSysVersion() {
  const cargoLock = readFileSync(cargoLockPath, 'utf8');
  const packageBlocks = cargoLock.split('[[package]]');
  for (const block of packageBlocks) {
    if (/^\s*name = "libduckdb-sys"\s*$/m.test(block)) {
      const match = block.match(/^\s*version = "([^"]+)"\s*$/m);
      if (!match) {
        throw new Error('Cargo.lock 中的 libduckdb-sys 缺少 version 字段。');
      }
      return match[1];
    }
  }

  throw new Error('Cargo.lock 中未找到 libduckdb-sys。');
}

function duckdbVersionFromCrateVersion(crateVersion) {
  const parts = crateVersion.split('.');
  const encoded = Number.parseInt(parts[1] ?? '', 10);
  if (!Number.isFinite(encoded)) {
    throw new Error(`无法从 libduckdb-sys 版本 ${crateVersion} 解析 DuckDB 版本。`);
  }

  const major = Math.floor(encoded / 10000);
  const minor = Math.floor(encoded / 100) % 100;
  const patch = encoded % 100;
  return `${major}.${minor}.${patch}`;
}

function hasRequiredFiles(downloadDirectory, requiredFiles) {
  return requiredFiles.every((fileName) => existsSync(join(downloadDirectory, fileName)));
}

async function downloadWithRetries(url, outputPath) {
  const retryCount = 8;
  let lastError;

  for (let attempt = 1; attempt <= retryCount; attempt += 1) {
    const temporaryPath = `${outputPath}.download`;
    try {
      rmSync(temporaryPath, { force: true });
      await download(url, temporaryPath);
      rmSync(outputPath, { force: true });
      renameSync(temporaryPath, outputPath);
      return;
    } catch (error) {
      rmSync(temporaryPath, { force: true });
      lastError = error;
      if (attempt < retryCount) {
        const delayMs = attempt * 5000;
        logger.warn(`DuckDB 下载失败，${delayMs / 1000} 秒后重试 (${attempt}/${retryCount})：${error.message}`);
        await delay(delayMs);
      }
    }
  }

  throw lastError;
}

function download(url, outputPath, redirectCount = 0) {
  if (redirectCount > 5) {
    return Promise.reject(new Error(`DuckDB 下载重定向次数过多：${url}`));
  }

  return new Promise((resolveDownload, rejectDownload) => {
    const client = url.startsWith('https:') ? https : http;
    const request = client.get(url, {
      headers: {
        'user-agent': 'revier-ci-duckdb-prewarm',
      },
    }, (response) => {
      const statusCode = response.statusCode ?? 0;
      if (statusCode >= 300 && statusCode < 400 && response.headers.location) {
        response.resume();
        const nextUrl = new URL(response.headers.location, url).toString();
        download(nextUrl, outputPath, redirectCount + 1).then(resolveDownload, rejectDownload);
        return;
      }

      if (statusCode < 200 || statusCode >= 300) {
        response.resume();
        rejectDownload(new Error(`HTTP ${statusCode}: ${url}`));
        return;
      }

      const file = createWriteStream(outputPath);
      response.pipe(file);
      response.on('error', rejectDownload);
      file.on('finish', () => file.close((error) => {
        if (error) {
          rejectDownload(error);
        } else {
          resolveDownload();
        }
      }));
      file.on('error', rejectDownload);
    });

    request.setTimeout(600000, () => {
      request.destroy(new Error(`DuckDB 下载超时：${url}`));
    });
    request.on('error', rejectDownload);
  });
}

function extractZip(archivePath, destination) {
  const command = process.platform === 'win32' ? 'powershell.exe' : 'unzip';
  const args = process.platform === 'win32'
    ? [
        '-NoLogo',
        '-NoProfile',
        '-ExecutionPolicy',
        'Bypass',
        '-Command',
        '& { param($archivePath, $destination) Expand-Archive -LiteralPath $archivePath -DestinationPath $destination -Force }',
        archivePath,
        destination,
      ]
    : ['-q', '-o', archivePath, '-d', destination];

  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`DuckDB 压缩包解压失败，退出码：${result.status}`);
  }
}

async function prewarmTarget(target, duckdbVersion) {
  const archive = archiveForTarget(target);
  if (!archive) {
    throw new Error(`当前目标没有可下载的 DuckDB 预编译库：${target}`);
  }

  const downloadDirectory = join(cargoTargetDirectory(), 'duckdb-download', target, duckdbVersion);
  const archivePath = join(downloadDirectory, archive.archiveName);
  mkdirSync(downloadDirectory, { recursive: true });

  if (hasRequiredFiles(downloadDirectory, archive.requiredFiles)) {
    logger.log(`已复用 DuckDB 缓存：${downloadDirectory}`);
    return;
  }

  if (!existsSync(archivePath)) {
    const url = `https://github.com/duckdb/duckdb/releases/download/v${duckdbVersion}/${archive.archiveName}`;
    const cachedArchivePath = downloadedArchives.get(url);
    if (cachedArchivePath && existsSync(cachedArchivePath)) {
      copyFileSync(cachedArchivePath, archivePath);
      logger.log(`已复用本轮下载的 DuckDB 压缩包：${archivePath}`);
    } else {
      logger.log(`正在下载 DuckDB ${duckdbVersion}：${url}`);
      await downloadWithRetries(url, archivePath);
      downloadedArchives.set(url, archivePath);
    }
  }

  logger.log(`正在解压 DuckDB：${archivePath}`);
  extractZip(archivePath, downloadDirectory);

  if (!hasRequiredFiles(downloadDirectory, archive.requiredFiles)) {
    throw new Error(`DuckDB 缓存缺少必要文件：${archive.requiredFiles.join(', ')}`);
  }
  logger.log(`已预热 DuckDB 缓存：${downloadDirectory}`);
}

if (targets.length === 0) {
  logger.error('用法：node scripts/prewarm-duckdb.mjs <target-triple> [target-triple...]');
  process.exit(2);
}

const crateVersion = readLibduckdbSysVersion();
const duckdbVersion = duckdbVersionFromCrateVersion(crateVersion);

for (const target of targets) {
  await prewarmTarget(target, duckdbVersion);
}
