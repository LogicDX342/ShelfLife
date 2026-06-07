use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum OriginEvidence {
    MacWhereFroms {
        values: Vec<String>,
    },
    WindowsZoneIdentifier {
        zone_id: Option<u32>,
        host_url: Option<String>,
        referrer_url: Option<String>,
    },
    LinuxXattr {
        key: String,
        value_utf8: Option<String>,
    },
    Unknown,
}
