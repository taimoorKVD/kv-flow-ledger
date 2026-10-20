/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: ["class"],
  theme: {
    extend: {
      colors: {
        ink: "var(--ink)",
        muted: "var(--muted)",
        stroke: "var(--stroke)",
        surface: "var(--surface)",
        canvas: "var(--bg)",
        accent: "var(--accent)",
      },
      fontFamily: {
        sans: [
          "IBM Plex Sans",
          "Segoe UI",
          "ui-sans-serif",
          "system-ui",
          "sans-serif",
        ],
        mono: ["IBM Plex Mono", "ui-monospace", "monospace"],
      },
      boxShadow: {
        glow: "0 12px 60px #0b4b8930",
      },
    },
  },
  plugins: [],
};
