import type { BuildArgs } from './args.ts';

// One JSON message per line over stdin/stdout. Mirrored by cli/src/builder/server.rs.
export type Request =
  | { id: number; command: 'build'; payload: BuildArgs }
  | { id: number; command: 'shutdown' };

export interface Response {
  id: number | null;
  ok: boolean;
  error?: string;
  result?: unknown;
}
