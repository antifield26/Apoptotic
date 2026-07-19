//! Prometheus metrics 导出
//!
//! 提供服务器运行指标（玩家数、TPS、内存使用等）。

/// Metrics 导出器
pub struct MetricsExporter {
    enabled: bool,
}

impl MetricsExporter {
    pub fn new(enabled: bool, _port: u16) -> Self {
        Self { enabled }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
