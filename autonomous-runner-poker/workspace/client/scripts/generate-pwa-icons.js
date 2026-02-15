/**
 * PWA Icon Generation Script
 * Run with: node scripts/generate-pwa-icons.js
 *
 * This creates placeholder icons. For production, use a proper icon generator
 * or design tool to create poker-themed icons at the required sizes.
 */

import { writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const publicDir = join(__dirname, '..', 'public');

// Simple SVG template for poker icon
const createPokerIcon = (size, maskable = false) => {
  const padding = maskable ? size * 0.1 : 0;
  const innerSize = size - padding * 2;
  const scale = innerSize / 192;

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
  <defs>
    <linearGradient id="bgGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#0d2818"/>
      <stop offset="100%" style="stop-color:#1a4528"/>
    </linearGradient>
  </defs>
  ${maskable ? `<rect width="${size}" height="${size}" fill="#0d2818"/>` : ''}
  <g transform="translate(${padding},${padding}) scale(${scale})">
    <rect width="192" height="192" rx="24" fill="url(#bgGrad)"/>
    <rect x="8" y="8" width="176" height="176" rx="20" fill="none" stroke="#ffd700" stroke-width="2" opacity="0.3"/>

    <!-- Card -->
    <g transform="translate(96,85)">
      <rect x="-30" y="-40" width="60" height="80" rx="6" fill="white" stroke="#ccc" stroke-width="1"/>
      <!-- Spade -->
      <path d="M0,-22 C-10,-12 -18,-2 -18,10 C-18,18 -12,23 -6,23 C-2,23 0,20 0,20 C0,20 2,23 6,23 C12,23 18,18 18,10 C18,-2 10,-12 0,-22" fill="#1a1a1a"/>
      <rect x="-4" y="18" width="8" height="12" fill="#1a1a1a"/>
      <!-- A -->
      <text x="-22" y="-22" font-family="Georgia, serif" font-size="18" font-weight="bold" fill="#1a1a1a">A</text>
    </g>

    <!-- Chips -->
    <g transform="translate(145,145)">
      <ellipse cx="0" cy="0" rx="22" ry="13" fill="#c41e3a" stroke="#ffd700" stroke-width="2"/>
      <ellipse cx="0" cy="-6" rx="22" ry="13" fill="#1e4d2b" stroke="#ffd700" stroke-width="2"/>
      <ellipse cx="0" cy="-12" rx="22" ry="13" fill="#1a365d" stroke="#ffd700" stroke-width="2"/>
    </g>
  </g>
</svg>`;
};

// Generate icons
const icons = [
  { name: 'pwa-192x192.svg', size: 192, maskable: false },
  { name: 'pwa-512x512.svg', size: 512, maskable: false },
  { name: 'pwa-maskable-192x192.svg', size: 192, maskable: true },
  { name: 'pwa-maskable-512x512.svg', size: 512, maskable: true },
  { name: 'apple-touch-icon.svg', size: 180, maskable: false },
];

icons.forEach(({ name, size, maskable }) => {
  const svg = createPokerIcon(size, maskable);
  writeFileSync(join(publicDir, name), svg);
  console.log(`Created ${name}`);
});

console.log('\\nPWA icons generated successfully!');
console.log('Note: For production, convert SVGs to PNGs using a tool like sharp or Inkscape.');
