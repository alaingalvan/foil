import { join } from 'path';
import { stat } from 'fs/promises';

// Per-request build state: mode, tsconfig, webpack cache location.
export interface BuildContext {
  nodeEnvStr: 'production' | 'development';
  cacheDir: string;
  tsConfigFile: string;
}

// ⚙️ Find the best tsconfig file
export async function findTsConfig(rootPath: string) {
  const possibleTsConfigs = [
    join(rootPath, 'tsconfig.json'),
    join(process.cwd(), 'tsconfig.json')
  ];
  for (const configPath of possibleTsConfigs) {
    try {
      await stat(configPath);
      return configPath;
    } catch {
      // Try the next candidate.
    }
  }

  return '';
}
