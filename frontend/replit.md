# HyperDEX - Ultra-Fast DEX Aggregator

## Overview

HyperDEX is a high-performance decentralized exchange (DEX) aggregator built with a speed-first philosophy. The application provides users with the fastest possible token swapping experience by aggregating liquidity across multiple DEXes and optimizing for ultra-low latency interactions. The platform emphasizes performance metrics, competitive speed comparisons, and a premium user interface designed around the concept of velocity and instant execution.

## User Preferences

Preferred communication style: Simple, everyday language.

## System Architecture

### Frontend Architecture
- **Framework**: React 18 with TypeScript for type safety and modern development patterns
- **Styling**: Tailwind CSS with a custom speed-focused design system featuring electric lime (#39FF14), nuclear blue (#00D4FF), and lightning yellow (#FFFF00) color palette
- **Animations**: Framer Motion for hardware-accelerated animations with strict performance constraints (all transitions under 150ms)
- **UI Components**: Radix UI primitives with shadcn/ui components for accessibility and consistency
- **State Management**: TanStack React Query for server state management and caching
- **Build Tool**: Vite for fast development and optimized production builds

### Backend Architecture
- **Runtime**: Node.js with Express.js framework
- **Language**: TypeScript with ES modules for modern JavaScript features
- **API Design**: RESTful endpoints for token data, swap quotes, and performance metrics
- **Development**: Hot module replacement and middleware-based request logging

### Database & ORM
- **Database**: PostgreSQL with Neon serverless database provider
- **ORM**: Drizzle ORM for type-safe database operations and schema management
- **Migrations**: Drizzle Kit for database schema versioning and deployment
- **Schema**: Includes tables for users, tokens, swaps, and performance metrics with proper relationships

### Design System & UI Philosophy
- **Speed-First Design**: All interactions optimized for perceived performance with instant visual feedback
- **Typography**: Inter font family with italicized text suggesting forward motion, JetBrains Mono for numerical data
- **Color Strategy**: High-contrast colors (pure black background, electric accent colors) for maximum visual impact
- **Animation Principles**: Hardware acceleration using transform3d, custom easing functions, and motion blur effects
- **Layout**: Centered swap interface with performance sidebar displaying real-time metrics

### Performance Optimizations
- **Bundle Optimization**: Vite-based build system with code splitting and tree shaking
- **Asset Loading**: Font preloading and optimized resource hints
- **Runtime Performance**: Skeleton loading states matching exact final layouts
- **Animation Performance**: GPU-accelerated transforms with optimized frame rates
- **State Updates**: Optimistic UI updates with immediate visual feedback

## External Dependencies

### Core Framework Dependencies
- **React Ecosystem**: React 18, React DOM, React Hook Form with Zod validation
- **Routing**: Wouter for lightweight client-side routing
- **Styling**: Tailwind CSS with PostCSS and Autoprefixer

### UI & Animation Libraries
- **Component Library**: Complete Radix UI ecosystem for accessible primitives
- **Animation**: Framer Motion for performance-optimized animations
- **Icons**: Lucide React for consistent iconography
- **Utilities**: class-variance-authority, clsx for dynamic styling

### Database & Backend
- **Database**: Neon Database (PostgreSQL) with connection pooling
- **ORM**: Drizzle ORM with Zod schema validation
- **Session Management**: connect-pg-simple for PostgreSQL session storage

### Development & Build Tools
- **Build**: Vite with React plugin and TypeScript support
- **Development**: tsx for TypeScript execution, esbuild for production builds
- **Replit Integration**: Specialized plugins for Replit environment compatibility
- **Quality**: TypeScript compiler for type checking

### Utility Libraries
- **Date Handling**: date-fns for date manipulation and formatting
- **Command Interface**: cmdk for command palette functionality
- **Carousel**: Embla Carousel for touch-friendly content sliding
- **Validation**: Zod for runtime type validation and schema enforcement