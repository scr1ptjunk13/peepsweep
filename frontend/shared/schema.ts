import { sql } from "drizzle-orm";
import { pgTable, text, varchar, decimal, integer, timestamp, boolean } from "drizzle-orm/pg-core";
import { createInsertSchema } from "drizzle-zod";
import { z } from "zod";

export const users = pgTable("users", {
  id: varchar("id").primaryKey().default(sql`gen_random_uuid()`),
  username: text("username").notNull().unique(),
  password: text("password").notNull(),
});

export const tokens = pgTable("tokens", {
  id: varchar("id").primaryKey(),
  symbol: text("symbol").notNull(),
  name: text("name").notNull(),
  decimals: integer("decimals").notNull(),
  logoUrl: text("logo_url"),
  price: decimal("price", { precision: 18, scale: 8 }),
  priceChange24h: decimal("price_change_24h", { precision: 5, scale: 2 }),
  isPopular: boolean("is_popular").default(false),
});

export const swaps = pgTable("swaps", {
  id: varchar("id").primaryKey().default(sql`gen_random_uuid()`),
  userId: varchar("user_id").references(() => users.id),
  fromTokenId: varchar("from_token_id").references(() => tokens.id),
  toTokenId: varchar("to_token_id").references(() => tokens.id),
  fromAmount: decimal("from_amount", { precision: 36, scale: 18 }),
  toAmount: decimal("to_amount", { precision: 36, scale: 18 }),
  executionTime: integer("execution_time"), // in milliseconds
  gasUsed: decimal("gas_used", { precision: 18, scale: 0 }),
  slippage: decimal("slippage", { precision: 5, scale: 4 }),
  status: text("status").notNull(), // 'pending', 'completed', 'failed'
  createdAt: timestamp("created_at").default(sql`now()`),
});

export const performanceMetrics = pgTable("performance_metrics", {
  id: varchar("id").primaryKey().default(sql`gen_random_uuid()`),
  averageExecutionTime: integer("avg_execution_time"), // in ms
  successRate: decimal("success_rate", { precision: 5, scale: 4 }),
  totalVolume24h: decimal("total_volume_24h", { precision: 36, scale: 18 }),
  gasSavedTotal: decimal("gas_saved_total", { precision: 36, scale: 18 }),
  activeTraders: integer("active_traders"),
  timestamp: timestamp("timestamp").default(sql`now()`),
});

export const insertUserSchema = createInsertSchema(users).pick({
  username: true,
  password: true,
});

export const insertTokenSchema = createInsertSchema(tokens).omit({
  id: true,
});

export const insertSwapSchema = createInsertSchema(swaps).omit({
  id: true,
  createdAt: true,
});

export const insertPerformanceMetricsSchema = createInsertSchema(performanceMetrics).omit({
  id: true,
  timestamp: true,
});

export type InsertUser = z.infer<typeof insertUserSchema>;
export type User = typeof users.$inferSelect;
export type Token = typeof tokens.$inferSelect;
export type InsertToken = z.infer<typeof insertTokenSchema>;
export type Swap = typeof swaps.$inferSelect;
export type InsertSwap = z.infer<typeof insertSwapSchema>;
export type PerformanceMetrics = typeof performanceMetrics.$inferSelect;
export type InsertPerformanceMetrics = z.infer<typeof insertPerformanceMetricsSchema>;
