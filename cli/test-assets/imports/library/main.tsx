import type { CSSProperties } from 'react';
import { Icon } from 'alain-xyz';
import Description from './description.mdx';
import Hero from './components/hero';
import { clamp } from '../shared/util';
import './styles/base.scss';
import shader from '!!raw-loader!./shaders/main.glsl';
import compute from './shaders/compute.wgsl?raw';
// import Commented from './commented';
import Missing from './missing';

export * from './reexport';
export const lazy = () => import('./lazy');
const cjs = require('./cjs-helper.js');

export default () => <p>It wouldn't break the scanner</p>;
