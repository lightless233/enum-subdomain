/// 引擎的状态，只有三个
#[derive(Debug, PartialEq)]
pub enum EngineStatus {
    Init,
    Running,
    Stop,
}

/// App Context
/// 存储各个引擎的状态，以及 app args
pub struct AppContext {
    pub task_builder_status: EngineStatus,
    pub resolver_status: Vec<EngineStatus>,
    pub saver_status: EngineStatus,
}

impl AppContext {
    pub fn new() -> Self {
        Self {
            task_builder_status: EngineStatus::Init,
            resolver_status: vec![],
            saver_status: EngineStatus::Init,
        }
    }
}

#[derive(Debug, Default)]
pub struct ResolveResult {
    pub domain: String,
    pub title: Option<String>,
    pub code: Option<usize>,
    pub ip: Vec<String>,
    pub cname: Vec<String>,
}
