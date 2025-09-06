import { Variants } from "framer-motion";

// Speed-optimized animation configurations
export const speedAnimations = {
  // Ultra-fast button hover (under 100ms)
  buttonHover: {
    scale: 1.02,
    transition: { duration: 0.08, ease: "easeOut" }
  },

  // Lightning-fast button tap
  buttonTap: {
    scale: 0.98,
    transition: { duration: 0.05, ease: "easeOut" }
  },

  // Electric glow effect
  electricGlow: {
    boxShadow: [
      "0 0 20px rgba(57, 255, 20, 0.3)",
      "0 0 30px rgba(57, 255, 20, 0.5), 0 0 40px rgba(57, 255, 20, 0.3)",
      "0 0 20px rgba(57, 255, 20, 0.3)"
    ],
    transition: { duration: 2, ease: "easeInOut", repeat: Infinity }
  },

  // Number counter animation
  counterUpdate: {
    scale: [1, 1.1, 1],
    opacity: [0.8, 1, 1],
    transition: { duration: 0.3, ease: "easeOut" }
  },

  // Particle burst effect
  particleBurst: {
    scale: [1, 20],
    opacity: [1, 0],
    transition: { duration: 0.5, ease: "easeOut" }
  },

  // Lightning flash
  lightningFlash: {
    scale: [1, 1.05, 1],
    filter: ["brightness(1)", "brightness(1.5)", "brightness(1)"],
    transition: { duration: 0.1, ease: "easeOut" }
  }
};

// Modal animation variants
export const modalVariants: Variants = {
  hidden: {
    opacity: 0,
    scale: 0.95,
    y: 20
  },
  visible: {
    opacity: 1,
    scale: 1,
    y: 0,
    transition: {
      duration: 0.15,
      ease: "easeOut"
    }
  },
  exit: {
    opacity: 0,
    scale: 0.95,
    y: 20,
    transition: {
      duration: 0.1,
      ease: "easeIn"
    }
  }
};

// Backdrop animation variants
export const backdropVariants: Variants = {
  hidden: { opacity: 0 },
  visible: { opacity: 1, transition: { duration: 0.15 } },
  exit: { opacity: 0, transition: { duration: 0.1 } }
};

// Slide up animation for cards
export const slideUpVariants: Variants = {
  hidden: {
    opacity: 0,
    y: 20
  },
  visible: {
    opacity: 1,
    y: 0,
    transition: {
      duration: 0.3,
      ease: "easeOut"
    }
  }
};

// Slide in from left for lists
export const slideInVariants: Variants = {
  hidden: {
    opacity: 0,
    x: -20
  },
  visible: {
    opacity: 1,
    x: 0,
    transition: {
      duration: 0.2,
      ease: "easeOut"
    }
  }
};

// Stagger animation for lists
export const staggerContainer: Variants = {
  visible: {
    transition: {
      staggerChildren: 0.05
    }
  }
};

// Loading spinner animation
export const spinnerVariants: Variants = {
  animate: {
    rotate: 360,
    transition: {
      duration: 1,
      ease: "linear",
      repeat: Infinity
    }
  }
};

// Pulse animation for live indicators
export const pulseVariants: Variants = {
  animate: {
    opacity: [1, 0.5, 1],
    scale: [1, 1.05, 1],
    transition: {
      duration: 2,
      ease: "easeInOut",
      repeat: Infinity
    }
  }
};

// Speed meter animation
export const speedMeterVariants: Variants = {
  animate: (speed: number) => ({
    width: `${Math.max(10, 100 - (speed / 35) * 90)}%`,
    transition: {
      duration: 0.5,
      ease: "easeOut"
    }
  })
};

// Success animation
export const successVariants: Variants = {
  hidden: { scale: 0, opacity: 0 },
  visible: {
    scale: [0, 1.2, 1],
    opacity: [0, 1, 1],
    transition: {
      duration: 0.6,
      ease: "easeOut",
      times: [0, 0.6, 1]
    }
  }
};

// Error shake animation
export const errorShakeVariants: Variants = {
  shake: {
    x: [-5, 5, -5, 5, 0],
    transition: {
      duration: 0.4,
      ease: "easeInOut"
    }
  }
};

// Hardware acceleration helper
export const optimizeForPerformance = {
  style: {
    transform: "translateZ(0)", // Force hardware acceleration
    backfaceVisibility: "hidden" as const,
    perspective: 1000
  }
};

// Custom easing functions for speed theme
export const speedEasing = {
  lightning: [0.25, 0.46, 0.45, 0.94],
  electric: [0.4, 0, 0.2, 1],
  hyperfast: [0.8, 0, 0.2, 1]
};
