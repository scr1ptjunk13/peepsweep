import { useEffect, useRef } from "react";

interface UseMobileGesturesOptions {
  onSwipeUp?: () => void;
  onSwipeDown?: () => void;
  onSwipeLeft?: () => void;
  onSwipeRight?: () => void;
  onDoubleTap?: () => void;
  onLongPress?: () => void;
  threshold?: number; // Minimum distance for swipe
  timeThreshold?: number; // Maximum time for swipe
  longPressDelay?: number; // Delay for long press
}

export function useMobileGestures(options: UseMobileGesturesOptions) {
  const {
    onSwipeUp,
    onSwipeDown,
    onSwipeLeft,
    onSwipeRight,
    onDoubleTap,
    onLongPress,
    threshold = 50,
    timeThreshold = 500,
    longPressDelay = 500,
  } = options;

  const touchStart = useRef<{ x: number; y: number; time: number } | null>(null);
  const tapCount = useRef(0);
  const longPressTimer = useRef<NodeJS.Timeout | null>(null);

  const handleTouchStart = (e: TouchEvent) => {
    const touch = e.touches[0];
    touchStart.current = {
      x: touch.clientX,
      y: touch.clientY,
      time: Date.now(),
    };

    // Long press detection
    if (onLongPress) {
      longPressTimer.current = setTimeout(() => {
        onLongPress();
        // Haptic feedback for long press
        if (navigator.vibrate) {
          navigator.vibrate(50);
        }
      }, longPressDelay);
    }

    // Double tap detection
    if (onDoubleTap) {
      tapCount.current++;
      if (tapCount.current === 1) {
        setTimeout(() => {
          tapCount.current = 0;
        }, 300);
      } else if (tapCount.current === 2) {
        onDoubleTap();
        tapCount.current = 0;
        // Haptic feedback for double tap
        if (navigator.vibrate) {
          navigator.vibrate([10, 10, 10]);
        }
      }
    }
  };

  const handleTouchEnd = (e: TouchEvent) => {
    // Clear long press timer
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }

    if (!touchStart.current) return;

    const touch = e.changedTouches[0];
    const deltaX = touch.clientX - touchStart.current.x;
    const deltaY = touch.clientY - touchStart.current.y;
    const deltaTime = Date.now() - touchStart.current.time;
    const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);

    // Check if it's a swipe (distance > threshold and within time limit)
    if (distance > threshold && deltaTime < timeThreshold) {
      const angle = Math.atan2(deltaY, deltaX) * (180 / Math.PI);

      // Determine swipe direction based on angle
      if (Math.abs(angle) < 45) {
        // Right swipe
        onSwipeRight?.();
        vibratePattern([20]);
      } else if (Math.abs(angle) > 135) {
        // Left swipe
        onSwipeLeft?.();
        vibratePattern([20]);
      } else if (angle > 45 && angle < 135) {
        // Down swipe
        onSwipeDown?.();
        vibratePattern([20]);
      } else if (angle < -45 && angle > -135) {
        // Up swipe
        onSwipeUp?.();
        vibratePattern([20]);
      }
    }

    touchStart.current = null;
  };

  const vibratePattern = (pattern: number[]) => {
    if (navigator.vibrate) {
      navigator.vibrate(pattern);
    }
  };

  useEffect(() => {
    // Only add touch listeners on touch-enabled devices
    if ('ontouchstart' in window) {
      document.addEventListener('touchstart', handleTouchStart, { passive: true });
      document.addEventListener('touchend', handleTouchEnd, { passive: true });

      return () => {
        document.removeEventListener('touchstart', handleTouchStart);
        document.removeEventListener('touchend', handleTouchEnd);
      };
    }
  }, [
    onSwipeUp,
    onSwipeDown,
    onSwipeLeft,
    onSwipeRight,
    onDoubleTap,
    onLongPress,
    threshold,
    timeThreshold,
    longPressDelay,
  ]);

  return {
    // Helper function to add touch feedback to buttons
    addTouchFeedback: (element: HTMLElement) => {
      const handleTouchStart = () => {
        element.style.transform = 'scale(0.98)';
        vibratePattern([10]);
      };

      const handleTouchEnd = () => {
        element.style.transform = '';
      };

      element.addEventListener('touchstart', handleTouchStart, { passive: true });
      element.addEventListener('touchend', handleTouchEnd, { passive: true });
      element.addEventListener('touchcancel', handleTouchEnd, { passive: true });

      return () => {
        element.removeEventListener('touchstart', handleTouchStart);
        element.removeEventListener('touchend', handleTouchEnd);
        element.removeEventListener('touchcancel', handleTouchEnd);
      };
    }
  };
}
