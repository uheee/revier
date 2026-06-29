use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpikeDecision {
    GixOnly,
    AlgorithmAdjustmentRequired,
    FallbackRisk,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Serialize)]
pub struct SpikeDifference {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Serialize)]
pub struct SpikeCheck {
    pub name: String,
    pub status: CheckStatus,
    pub differences: Vec<SpikeDifference>,
}

#[derive(Debug, Serialize)]
pub struct SpikeOutput {
    pub version: u8,
    pub fixture: String,
    pub checks: Vec<SpikeCheck>,
    pub decision: SpikeDecision,
}

impl SpikeOutput {
    pub fn new(fixture: impl Into<String>) -> Self {
        Self {
            version: 1,
            fixture: fixture.into(),
            checks: Vec::new(),
            decision: SpikeDecision::AlgorithmAdjustmentRequired,
        }
    }
}
