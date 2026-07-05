#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { Console } from 'node:console';
import process from 'node:process';

const logger = new Console({ stdout: process.stdout, stderr: process.stderr });
const tagName = process.argv[2];

if (!tagName) {
  logger.error('用法：node scripts/set-package-version.mjs <release-tag>');
  process.exit(2);
}

const version = tagName.replace(/^v/, '');
if (!/^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$/.test(version)) {
  logger.error(`无效版本号：${tagName}`);
  process.exit(2);
}

const packageJsonPath = 'package.json';
const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
packageJson.version = version;
writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);

logger.log(`package version: ${version}`);
