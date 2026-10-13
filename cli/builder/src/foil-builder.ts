import { argv } from 'process';
import { serve } from './server.ts';
import { runOnce } from './one-shot.ts';

if (argv.includes('--serve')) {
  await serve();
} else {
  await runOnce(argv);
}
