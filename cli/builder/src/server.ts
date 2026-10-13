import { createInterface } from 'readline';
import { buildFoil } from './builder/foil.ts';
import type { Request, Response } from './protocol.ts';

// Server mode: one JSON request per stdin line, one JSON response per stdout line.
export async function serve() {
  const writeOut = process.stdout.write.bind(process.stdout);
  // stdout is the protocol channel, so route logs to stderr.
  console.log = console.error;
  console.info = console.error;

  const respond = (res: Response, done?: () => void) =>
    writeOut(JSON.stringify(res) + '\n', done);
  const pending = new Set<Promise<boolean>>();
  const rl = createInterface({ input: process.stdin });

  rl.on('line', (line) => {
    if (line.trim().length === 0) return;
    let req: Request;
    try {
      req = JSON.parse(line);
    } catch {
      respond({ id: null, ok: false, error: 'Invalid JSON request.' });
      return;
    }
    const id = req.id ?? null;
    if (req.command === 'shutdown') {
      Promise.allSettled([...pending]).then(() =>
        respond({ id, ok: true, result: null }, () => process.exit(0))
      );
    } else if (req.command === 'build') {
      const job = buildFoil(req.payload)
        .then(() => respond({ id, ok: true, result: null }))
        .catch((e) =>
          respond({ id, ok: false, error: String(e?.message ?? e) })
        )
        .finally(() => pending.delete(job));
      pending.add(job);
    } else {
      respond({
        id,
        ok: false,
        error: `Unknown command: ${(req as any).command}`
      });
    }
  });
  // The CLI exiting (including Ctrl+C) closes stdin.
  rl.on('close', async () => {
    await Promise.allSettled([...pending]);
    process.exit(0);
  });
}
