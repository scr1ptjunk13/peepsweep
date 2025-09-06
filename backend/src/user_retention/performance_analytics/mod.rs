pub mod user_analyzer;
pub mod comparative_analytics;
pub mod insights_generator;
pub mod reporter;

pub use user_analyzer::{UserPerformanceAnalyzer, UserPerformanceMetrics, TradingPattern, UserTrade};
pub use comparative_analytics::ComparativeAnalytics;
pub use insights_generator::InsightsGenerator;
pub use reporter::PerformanceReporter;
