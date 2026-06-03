import packageJson from '../../package.json';

describe('project scaffold', () => {
  it('uses the expected package name and Electron entry', () => {
    expect(packageJson.name).toBe('revier');
    expect(packageJson.main).toBe('dist/main/index.js');
  });
});
