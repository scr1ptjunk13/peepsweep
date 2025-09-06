import type { Config } from "tailwindcss";

export default {
  darkMode: ["class"],
  content: ["./client/index.html", "./client/src/**/*.{js,jsx,ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "var(--background)",
        foreground: "var(--foreground)",
        card: {
          DEFAULT: "var(--card)",
          foreground: "var(--card-foreground)",
        },
        popover: {
          DEFAULT: "var(--popover)",
          foreground: "var(--popover-foreground)",
        },
        primary: {
          DEFAULT: "var(--primary)",
          foreground: "var(--primary-foreground)",
        },
        secondary: {
          DEFAULT: "var(--secondary)",
          foreground: "var(--secondary-foreground)",
        },
        muted: {
          DEFAULT: "var(--muted)",
          foreground: "var(--muted-foreground)",
        },
        accent: {
          DEFAULT: "var(--accent)",
          foreground: "var(--accent-foreground)",
        },
        destructive: {
          DEFAULT: "var(--destructive)",
          foreground: "var(--destructive-foreground)",
        },
        success: {
          DEFAULT: "var(--success)",
          foreground: "var(--success-foreground)",
        },
        border: "var(--border)",
        input: "var(--input)",
        ring: "var(--ring)",
        chart: {
          "1": "var(--chart-1)",
          "2": "var(--chart-2)",
          "3": "var(--chart-3)",
          "4": "var(--chart-4)",
          "5": "var(--chart-5)",
        },
        sidebar: {
          DEFAULT: "var(--sidebar-background)",
          foreground: "var(--sidebar-foreground)",
          primary: "var(--sidebar-primary)",
          "primary-foreground": "var(--sidebar-primary-foreground)",
          accent: "var(--sidebar-accent)",
          "accent-foreground": "var(--sidebar-accent-foreground)",
          border: "var(--sidebar-border)",
          ring: "var(--sidebar-ring)",
        },
        // HyperDEX Speed Colors
        'electric-lime': '#39FF14',
        'nuclear-blue': '#00D4FF',
        'lightning-yellow': '#FFFF00',
        'velocity-green': '#00FF88',
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      fontFamily: {
        sans: ["Inter", "sans-serif"],
        mono: ["JetBrains Mono", "monospace"],
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
        "pulse-fast": "pulse 1s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "bounce-subtle": "bounce 1.5s infinite",
        "glow": "glow 2s ease-in-out infinite alternate",
        "counter": "counter 0.5s ease-out",
        "slide-up": "slideUp 0.3s ease-out",
        "lightning": "lightning 0.1s ease-out",
        "grid-move": "grid-move 20s linear infinite",
        "lightning-gradient": "lightning-gradient 2s ease infinite",
        "skeleton-loading": "skeleton-loading 1.5s infinite",
        "particle-burst": "particle-burst 0.5s ease-out",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
        glow: {
          'from': { boxShadow: '0 0 20px #39FF14' },
          'to': { boxShadow: '0 0 30px #39FF14, 0 0 40px #39FF14' }
        },
        counter: {
          'from': { transform: 'scale(1.1)', opacity: '0.8' },
          'to': { transform: 'scale(1)', opacity: '1' }
        },
        slideUp: {
          'from': { transform: 'translateY(10px)', opacity: '0' },
          'to': { transform: 'translateY(0)', opacity: '1' }
        },
        lightning: {
          '0%': { transform: 'scale(1)', filter: 'brightness(1)' },
          '50%': { transform: 'scale(1.05)', filter: 'brightness(1.5)' },
          '100%': { transform: 'scale(1)', filter: 'brightness(1)' }
        },
        "grid-move": {
          '0%': { transform: 'translate(0, 0)' },
          '100%': { transform: 'translate(50px, 50px)' }
        },
        "lightning-gradient": {
          '0%': { backgroundPosition: '0% 50%' },
          '50%': { backgroundPosition: '100% 50%' },
          '100%': { backgroundPosition: '0% 50%' }
        },
        "skeleton-loading": {
          '0%': { backgroundPosition: '200% 0' },
          '100%': { backgroundPosition: '-200% 0' }
        },
        "particle-burst": {
          '0%': { 
            opacity: '1', 
            transform: 'translate(-50%, -50%) scale(1)'
          },
          '100%': { 
            opacity: '0', 
            transform: 'translate(-50%, -50%) scale(20)'
          }
        },
      },
      transitionTimingFunction: {
        'speed': 'cubic-bezier(0.25, 0.46, 0.45, 0.94)',
      },
      transitionDuration: {
        'speed': '100ms',
      }
    },
  },
  plugins: [require("tailwindcss-animate"), require("@tailwindcss/typography")],
} satisfies Config;
