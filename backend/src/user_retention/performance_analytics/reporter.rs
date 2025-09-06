use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rust_decimal::Decimal;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

use crate::user_retention::performance_analytics::user_analyzer::{UserPerformanceMetrics, TradingPattern, UserGrowthMetrics};
use crate::user_retention::performance_analytics::comparative_analytics::{MarketComparison, BenchmarkComparison, PeerComparison};
use crate::user_retention::performance_analytics::insights_generator::{TradingInsight, PerformanceRecommendation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub report_id: Uuid,
    pub user_id: Uuid,
    pub report_type: ReportType,
    pub time_period: ReportPeriod,
    pub generated_at: DateTime<Utc>,
    pub summary: ReportSummary,
    pub detailed_metrics: UserPerformanceMetrics,
    pub growth_analysis: UserGrowthMetrics,
    pub market_comparison: Option<MarketComparison>,
    pub insights: Vec<TradingInsight>,
    pub recommendations: Option<PerformanceRecommendation>,
    pub charts_data: Vec<ChartData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportType {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Annual,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPeriod {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub duration_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_return: Decimal,
    pub win_rate: f64,
    pub total_trades: u64,
    pub best_trade: Decimal,
    pub worst_trade: Decimal,
    pub sharpe_ratio: f64,
    pub max_drawdown: Decimal,
    pub portfolio_value: Decimal,
    pub peer_percentile: f64,
    pub key_highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub chart_type: ChartType,
    pub title: String,
    pub data_points: Vec<DataPoint>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChartType {
    PortfolioValue,
    Returns,
    Drawdown,
    WinRate,
    TradeFrequency,
    DexPerformance,
    TokenAllocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportableReport {
    pub format: ExportFormat,
    pub content: Vec<u8>,
    pub filename: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    PDF,
    CSV,
    JSON,
    Excel,
}

pub struct PerformanceReporter {
    reports_cache: Arc<RwLock<HashMap<Uuid, PerformanceReport>>>,
}

impl PerformanceReporter {
    pub fn new() -> Self {
        Self {
            reports_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a comprehensive performance report
    pub async fn generate_report(
        &self,
        user_id: Uuid,
        report_type: ReportType,
        user_metrics: &UserPerformanceMetrics,
        growth_metrics: &UserGrowthMetrics,
        trading_pattern: &TradingPattern,
        market_comparison: Option<MarketComparison>,
        insights: Vec<TradingInsight>,
        recommendations: Option<PerformanceRecommendation>,
    ) -> Result<PerformanceReport, Box<dyn std::error::Error + Send + Sync>> {
        let report_period = self.calculate_report_period(&report_type);
        
        // Generate summary
        let summary = self.generate_summary(
            user_metrics,
            growth_metrics,
            &market_comparison,
            &insights,
        );

        // Generate chart data
        let charts_data = self.generate_charts_data(
            user_metrics,
            growth_metrics,
            trading_pattern,
            &market_comparison,
        ).await?;

        let report = PerformanceReport {
            report_id: Uuid::new_v4(),
            user_id,
            report_type,
            time_period: report_period,
            generated_at: Utc::now(),
            summary,
            detailed_metrics: user_metrics.clone(),
            growth_analysis: growth_metrics.clone(),
            market_comparison,
            insights,
            recommendations,
            charts_data,
        };

        // Cache the report
        let mut cache = self.reports_cache.write().await;
        cache.insert(report.report_id, report.clone());

        Ok(report)
    }

    /// Export report to different formats
    pub async fn export_report(
        &self,
        report: &PerformanceReport,
        format: ExportFormat,
    ) -> Result<ExportableReport, Box<dyn std::error::Error + Send + Sync>> {
        let content = match format {
            ExportFormat::JSON => self.export_to_json(report)?,
            ExportFormat::CSV => self.export_to_csv(report)?,
            ExportFormat::PDF => self.export_to_pdf(report).await?,
            ExportFormat::Excel => self.export_to_excel(report)?,
        };

        let filename = self.generate_filename(report, &format);

        Ok(ExportableReport {
            format,
            content,
            filename,
            generated_at: Utc::now(),
        })
    }

    // Private helper methods
    fn calculate_report_period(&self, report_type: &ReportType) -> ReportPeriod {
        let end_date = Utc::now();
        let (start_date, duration_days) = match report_type {
            ReportType::Daily => (end_date - Duration::days(1), 1),
            ReportType::Weekly => (end_date - Duration::days(7), 7),
            ReportType::Monthly => (end_date - Duration::days(30), 30),
            ReportType::Quarterly => (end_date - Duration::days(90), 90),
            ReportType::Annual => (end_date - Duration::days(365), 365),
            ReportType::Custom => (end_date - Duration::days(30), 30), // Default to monthly
        };

        ReportPeriod {
            start_date,
            end_date,
            duration_days,
        }
    }

    fn generate_summary(
        &self,
        user_metrics: &UserPerformanceMetrics,
        growth_metrics: &UserGrowthMetrics,
        market_comparison: &Option<MarketComparison>,
        insights: &[TradingInsight],
    ) -> ReportSummary {
        let peer_percentile = market_comparison
            .as_ref()
            .map(|mc| mc.peer_comparison.user_percentile)
            .unwrap_or(50.0);

        let key_highlights = self.generate_key_highlights(user_metrics, growth_metrics, insights);

        ReportSummary {
            total_return: user_metrics.total_return,
            win_rate: user_metrics.win_rate,
            total_trades: user_metrics.total_trades,
            best_trade: user_metrics.largest_win,
            worst_trade: user_metrics.largest_loss,
            sharpe_ratio: user_metrics.sharpe_ratio,
            max_drawdown: user_metrics.max_drawdown,
            portfolio_value: user_metrics.portfolio_value,
            peer_percentile,
            key_highlights,
        }
    }

    fn generate_key_highlights(
        &self,
        user_metrics: &UserPerformanceMetrics,
        growth_metrics: &UserGrowthMetrics,
        insights: &[TradingInsight],
    ) -> Vec<String> {
        let mut highlights = Vec::new();

        // Performance highlights
        if user_metrics.total_return > Decimal::ZERO {
            highlights.push(format!("Positive return of {:.2}%", user_metrics.total_return * Decimal::from(100)));
        }

        if user_metrics.win_rate > 60.0 {
            highlights.push(format!("Strong win rate of {:.1}%", user_metrics.win_rate));
        }

        if user_metrics.sharpe_ratio > 1.0 {
            highlights.push(format!("Excellent risk-adjusted returns (Sharpe: {:.2})", user_metrics.sharpe_ratio));
        }

        // Growth highlights
        if growth_metrics.growth_percentage > Decimal::from(10) {
            highlights.push(format!("Portfolio grew by {:.1}%", growth_metrics.growth_percentage));
        }

        // Critical insights
        let critical_insights = insights.iter()
            .filter(|i| matches!(i.priority, crate::user_retention::performance_analytics::insights_generator::InsightPriority::Critical))
            .count();

        if critical_insights > 0 {
            highlights.push(format!("{} critical insights require attention", critical_insights));
        }

        highlights
    }

    async fn generate_charts_data(
        &self,
        user_metrics: &UserPerformanceMetrics,
        growth_metrics: &UserGrowthMetrics,
        trading_pattern: &TradingPattern,
        market_comparison: &Option<MarketComparison>,
    ) -> Result<Vec<ChartData>, Box<dyn std::error::Error + Send + Sync>> {
        let mut charts = Vec::new();

        // Portfolio value chart
        charts.push(ChartData {
            chart_type: ChartType::PortfolioValue,
            title: "Portfolio Value Over Time".to_string(),
            data_points: vec![
                DataPoint {
                    timestamp: Utc::now() - Duration::days(30),
                    value: growth_metrics.initial_portfolio_value,
                    label: Some("Start".to_string()),
                },
                DataPoint {
                    timestamp: Utc::now(),
                    value: growth_metrics.current_portfolio_value,
                    label: Some("Current".to_string()),
                },
            ],
            metadata: HashMap::new(),
        });

        // Returns chart
        charts.push(ChartData {
            chart_type: ChartType::Returns,
            title: "Monthly Returns".to_string(),
            data_points: growth_metrics.monthly_growth_rates.iter().enumerate()
                .map(|(i, &rate)| DataPoint {
                    timestamp: Utc::now() - Duration::days(30 * (growth_metrics.monthly_growth_rates.len() - i) as i64),
                    value: rate,
                    label: None,
                })
                .collect(),
            metadata: HashMap::new(),
        });

        Ok(charts)
    }

    fn export_to_json(&self, report: &PerformanceReport) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json = serde_json::to_string_pretty(report)?;
        Ok(json.into_bytes())
    }

    fn export_to_csv(&self, report: &PerformanceReport) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let mut csv = String::new();
        csv.push_str("Metric,Value\n");
        csv.push_str(&format!("Total Return,{:.2}%\n", report.summary.total_return * Decimal::from(100)));
        csv.push_str(&format!("Win Rate,{:.1}%\n", report.summary.win_rate));
        csv.push_str(&format!("Total Trades,{}\n", report.summary.total_trades));
        csv.push_str(&format!("Sharpe Ratio,{:.2}\n", report.summary.sharpe_ratio));
        csv.push_str(&format!("Max Drawdown,{:.2}%\n", report.summary.max_drawdown * Decimal::from(100)));
        csv.push_str(&format!("Portfolio Value,${:.2}\n", report.summary.portfolio_value));
        
        Ok(csv.into_bytes())
    }

    async fn export_to_pdf(&self, _report: &PerformanceReport) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // PDF generation would require a PDF library like printpdf
        // For now, return placeholder
        Ok(b"PDF content placeholder".to_vec())
    }

    fn export_to_excel(&self, _report: &PerformanceReport) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Excel generation would require a library like rust_xlsxwriter
        // For now, return placeholder
        Ok(b"Excel content placeholder".to_vec())
    }

    fn generate_filename(&self, report: &PerformanceReport, format: &ExportFormat) -> String {
        let extension = match format {
            ExportFormat::PDF => "pdf",
            ExportFormat::CSV => "csv",
            ExportFormat::JSON => "json",
            ExportFormat::Excel => "xlsx",
        };

        let report_type = match report.report_type {
            ReportType::Daily => "daily",
            ReportType::Weekly => "weekly",
            ReportType::Monthly => "monthly",
            ReportType::Quarterly => "quarterly",
            ReportType::Annual => "annual",
            ReportType::Custom => "custom",
        };

        format!("performance_report_{}_{}.{}", 
                report_type, 
                report.generated_at.format("%Y%m%d"), 
                extension)
    }

    /// Get cached report
    pub async fn get_cached_report(&self, report_id: Uuid) -> Option<PerformanceReport> {
        let cache = self.reports_cache.read().await;
        cache.get(&report_id).cloned()
    }
}

impl Default for PerformanceReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_reporter_creation() {
        let reporter = PerformanceReporter::new();
        assert!(reporter.reports_cache.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_report_generation() {
        // Test report generation logic
    }

    #[tokio::test]
    async fn test_export_formats() {
        // Test different export formats
    }
}
