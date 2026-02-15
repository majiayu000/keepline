/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        felt: {
          DEFAULT: '#1a472a',
          dark: '#0d2818',
          light: '#2d5a3d',
        },
        poker: {
          gold: '#ffd700',
          red: '#c41e3a',
          black: '#1a1a1a',
        },
      },
      fontFamily: {
        poker: ['Georgia', 'serif'],
      },
      screens: {
        'xs': '375px',
        // sm: 640px (default)
        // md: 768px (default)
        // lg: 1024px (default)
      },
    },
  },
  plugins: [],
}
