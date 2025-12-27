/**
 * Simple Icon Generator for Claude Quota Menubar
 *
 * Creates a basic menubar icon using Node.js canvas
 * Run: node scripts/generate-icon.js
 *
 * For production, replace with a properly designed icon.
 */

const fs = require('fs');
const path = require('path');

// Simple 16x16 PNG icon data (a gray circle - Claude-like)
// This is a minimal valid PNG file
const createSimpleIcon = () => {
  // 16x16 gray circle icon encoded as base64 PNG
  // This is a pre-generated minimal icon
  const iconBase64 = 'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAABHNCSVQICAgIfAhkiAAAAAlwSFlzAAAAdgAAAHYBTnsmCAAAABl0RVh0U29mdHdhcmUAd3d3Lmlua3NjYXBlLm9yZ5vuPBoAAADVSURBVDiNpZMxDoMwDEWfO3ABJDqyZGNkZOQQnKUrJ+AEnIATsHIFRkZGJE5QJHfAwlBRqkrtL0V2/vyTZ8cGRIRVkXEFALj7AgC4ewQAiAgA4O4LAODuEQAgIgCAuy8AgLtHAICIAADuvgAA7h4BACICALj7AgC4ewQAiAgA4O4LAODuEQAgIgCAuy8AgLtHAICIAADuvgAA7h4BACICALj7AgC4ewQAiAgA4O4LAODuEQAgIgCAuy8AgLtHAICIAGA/wl/8A34RAMDdlxXyAweCSMwC4TQ8AAAAAElFTkSuQmCC';

  return Buffer.from(iconBase64, 'base64');
};

// Create assets directory if it doesn't exist
const assetsDir = path.join(__dirname, '../assets');
if (!fs.existsSync(assetsDir)) {
  fs.mkdirSync(assetsDir, { recursive: true });
}

// Write the icon
const iconPath = path.join(assetsDir, 'iconTemplate.png');
fs.writeFileSync(iconPath, createSimpleIcon());

console.log(`Icon created at: ${iconPath}`);
console.log('Note: Replace this with a properly designed icon for production.');
