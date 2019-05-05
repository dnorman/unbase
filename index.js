// Note that a dynamic `import` statement here is required due to
// webpack/webpack#6615, but in theory `import { greet } from './pkg/hello_world';`
// will work here one day as well!
const rust = import('./pkg/unbase');

rust
    .then(m => m.greet('World!'))
.catch(console.error);
