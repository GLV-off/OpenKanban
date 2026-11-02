use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    auth::throttle::LoginThrottle,
    config::{Config, Environment},
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub throttle: LoginThrottle,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: Config) -> Self {
        Self {
            pool,
            config: Arc::new(config),
            throttle: LoginThrottle::new(),
        }
    }

    pub fn environment(&self) -> Environment {
        self.config.environment
    }
}
