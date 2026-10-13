import { argv } from 'process';
import { serve } from './server';
import { runOnce } from './one-shot';

if (argv.includes('--serve')) {
  await serve();
} else {
  await runOnce(argv);
}
