import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');

// Read the built index.html
let html = fs.readFileSync(path.join(root, 'ui', 'index.html'), 'utf8');

// Read the tauri-stub
const stub = fs.readFileSync(path.join(root, 'e2e-tests', 'tests', 'bridge', 'tauri-stub.js'), 'utf8');

// Inject the stub right before the closing </head> tag
const injectTag = '<script>' + stub + '</script>';
html = html.replace('</head>', injectTag + '\n</head>');

// Write the test HTML file
fs.writeFileSync(path.join(root, 'ui', 'test-e2e.html'), html);
console.log('Created ui/test-e2e.html with tauri-stub injected');
console.log('File size:', (html.length / 1024).toFixed(1) + 'KB');
