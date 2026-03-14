use std::sync::Arc;

use ferrobus_core::TransitModel;

#[derive(Clone)]
pub struct AppState {
    pub model: Arc<TransitModel>,
}

impl AppState {
    pub fn new(model: TransitModel) -> Self {
        Self {
            model: Arc::new(model),
        }
    }
}
