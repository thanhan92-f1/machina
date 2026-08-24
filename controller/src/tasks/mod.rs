// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The single NATS/local-bus subject every task is published and subscribed
/// on. Shared by `enqueue::enqueue_task`, `nats_subscriber::run`, and
/// `worker`'s retry re-publish — kept as one constant so the publisher and
/// subscriber can never drift apart on the subject string.
pub(crate) const TASK_SUBJECT: &str = "machina.tasks";

/// Message header `NatsTaskBus::publish` stamps with the publishing
/// controller's id, so `nats_subscriber` can recognize and drop the echo of
/// this controller's own publish instead of double-delivering it locally
/// (see `FanoutTaskBus::publish`'s doc comment for the full mechanism).
pub(crate) const ORIGIN_HEADER: &str = "X-Machina-Origin";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessage {
    pub task_id: Uuid,
    pub operation: String,
    pub payload: serde_json::Value,
}

#[async_trait]
pub trait TaskBus: Send + Sync {
    async fn publish(&self, subject: &str, msg: &TaskMessage) -> anyhow::Result<()>;
}

pub mod bus;
pub mod enqueue;
pub mod nats_subscriber;
pub mod worker;
