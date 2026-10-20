use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum SwitchManagementRequest {
    ProgramLoad(ProgramLoad),
    ProgramRemove(ProgramRemove),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramLoad {
    pub program_id: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramRemove {
    pub program_id: u16,
}
