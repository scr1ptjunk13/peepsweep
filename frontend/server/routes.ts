import type { Express } from "express";
import { createServer, type Server } from "http";
import { storage } from "./storage";
import { z } from "zod";
import { insertSwapSchema, insertPerformanceMetricsSchema } from "@shared/schema";

// Backend API proxy
const BACKEND_URL = process.env.BACKEND_URL || 'http://localhost:3000';

export async function registerRoutes(app: Express): Promise<Server> {
  // Get performance metrics
  app.get("/api/performance", async (req, res) => {
    try {
      const metrics = await storage.getLatestPerformanceMetrics();
      res.json(metrics);
    } catch (error) {
      res.status(500).json({ message: "Failed to fetch performance metrics" });
    }
  });

  // Get popular tokens
  app.get("/api/tokens/popular", async (req, res) => {
    try {
      const tokens = await storage.getPopularTokens();
      res.json(tokens);
    } catch (error) {
      res.status(500).json({ message: "Failed to fetch popular tokens" });
    }
  });

  // Search tokens
  app.get("/api/tokens/search", async (req, res) => {
    try {
      const { q } = req.query;
      if (!q || typeof q !== "string") {
        return res.status(400).json({ message: "Query parameter 'q' is required" });
      }
      const tokens = await storage.searchTokens(q);
      res.json(tokens);
    } catch (error) {
      res.status(500).json({ message: "Failed to search tokens" });
    }
  });

  // Get swap quote
  app.post("/api/quote", async (req, res) => {
    try {
      const quoteSchema = z.object({
        fromTokenId: z.string(),
        toTokenId: z.string(),
        amount: z.string(),
      });
      
      const { fromTokenId, toTokenId, amount } = quoteSchema.parse(req.body);
      const quote = await storage.getSwapQuote(fromTokenId, toTokenId, amount);
      res.json(quote);
    } catch (error) {
      if (error instanceof z.ZodError) {
        res.status(400).json({ message: "Invalid request data", errors: error.errors });
      } else {
        res.status(500).json({ message: "Failed to get swap quote" });
      }
    }
  });

  // Execute swap
  app.post("/api/swap", async (req, res) => {
    try {
      const validatedData = insertSwapSchema.parse(req.body);
      const swap = await storage.createSwap(validatedData);
      res.json(swap);
    } catch (error) {
      if (error instanceof z.ZodError) {
        res.status(400).json({ message: "Invalid swap data", errors: error.errors });
      } else {
        res.status(500).json({ message: "Failed to execute swap" });
      }
    }
  });

  // Get swap history
  app.get("/api/swaps", async (req, res) => {
    try {
      const { userId } = req.query;
      const swaps = userId 
        ? await storage.getUserSwaps(userId as string)
        : await storage.getRecentSwaps();
      res.json(swaps);
    } catch (error) {
      res.status(500).json({ message: "Failed to fetch swaps" });
    }
  });

  // Get live trading activity
  app.get("/api/activity/live", async (req, res) => {
    try {
      const activity = await storage.getLiveTradingActivity();
      res.json(activity);
    } catch (error) {
      res.status(500).json({ message: "Failed to fetch live activity" });
    }
  });

  // Update performance metrics (for simulation)
  app.post("/api/performance", async (req, res) => {
    try {
      const validatedData = insertPerformanceMetricsSchema.parse(req.body);
      const metrics = await storage.updatePerformanceMetrics(validatedData);
      res.json(metrics);
    } catch (error) {
      if (error instanceof z.ZodError) {
        res.status(400).json({ message: "Invalid metrics data", errors: error.errors });
      } else {
        res.status(500).json({ message: "Failed to update performance metrics" });
      }
    }
  });

  // Proxy to backend API for chain-abstraction endpoints
  app.use('/api/chain-abstraction/*', async (req, res) => {
    try {
      const backendUrl = `${BACKEND_URL}${req.originalUrl}`;
      const response = await fetch(backendUrl, {
        method: req.method,
        headers: {
          'Content-Type': 'application/json',
          'Accept': 'application/json'
        },
        body: req.method !== 'GET' ? JSON.stringify(req.body) : undefined
      });

      const data = await response.json();
      res.status(response.status).json(data);
    } catch (error) {
      console.error('Backend proxy error:', error);
      res.status(500).json({ message: 'Backend API unavailable' });
    }
  });

  const httpServer = createServer(app);
  return httpServer;
}
