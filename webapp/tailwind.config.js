/** @type {import('tailwindcss').Config} */
module.exports = {
  content: {
    // This tells Tailwind to scan your Rust files for class names
    files: ["*.html", "./src/**/*.rs"],
  },
  theme: {
    extend: {},
  },
  plugins: [],
}
