/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        moon: {
          50: '#f5f5f7',
          100: '#e8e8ed',
          200: '#d1d1d8',
          300: '#a8a8b3',
          400: '#7c7c8a',
          500: '#5a5a67',
          600: '#3d3d47',
          700: '#2a2a32',
          800: '#1a1a1f',
          900: '#0f0f12',
        }
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'Helvetica', 'Arial', 'sans-serif'],
      },
    },
  },
  plugins: [],
}
