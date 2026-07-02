import { spawnSync } from 'node:child_process';

const result = spawnSync('cargo', process.argv.slice(2), {
  env: {
    ...process.env,
    DUCKDB_DOWNLOAD_LIB: '1'
  },
  stdio: 'inherit'
});

if (result.error) {
  console.error(`无法执行 cargo：${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
