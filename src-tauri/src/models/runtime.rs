use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconciliationReport {
    pub indexed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
}
