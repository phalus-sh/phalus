//! Bridges symbi-runtime journal entries to phalus `ProgressEvent`s.
//!
//! [`ProgressJournalWriter`] implements the symbi-runtime [`JournalWriter`]
//! trait and converts each [`LoopEvent`] into a [`ProgressEvent::AgentIteration`]
//! that the web UI / CLI can consume via the broadcast channel.

use std::sync::atomic::{AtomicU64, Ordering};

use symbi_runtime::reasoning::loop_types::{
    JournalEntry, JournalError, JournalWriter, LoopEvent, TerminationReason,
};
use tokio::sync::broadcast;

use crate::pipeline::ProgressEvent;

/// A [`JournalWriter`] that maps journal entries to [`ProgressEvent::AgentIteration`]
/// events and sends them over a broadcast channel.
pub struct ProgressJournalWriter {
    package_name: String,
    max_iterations: u32,
    tx: Option<broadcast::Sender<ProgressEvent>>,
    sequence: AtomicU64,
}

impl ProgressJournalWriter {
    /// Create a new writer.
    ///
    /// If `tx` is `None`, journal entries are silently dropped (useful in
    /// headless / test contexts where no UI is listening).
    pub fn new(
        package_name: String,
        max_iterations: u32,
        tx: Option<broadcast::Sender<ProgressEvent>>,
    ) -> Self {
        Self {
            package_name,
            max_iterations,
            tx,
            sequence: AtomicU64::new(0),
        }
    }

    /// Convert a [`LoopEvent`] into a human-readable detail string.
    fn detail_for_event(event: &LoopEvent) -> String {
        match event {
            LoopEvent::Started { .. } => "starting reasoning loop".to_string(),
            LoopEvent::ReasoningComplete { actions, usage, .. } => {
                format!(
                    "reasoning complete, {} actions proposed ({} tokens)",
                    actions.len(),
                    usage.total_tokens
                )
            }
            LoopEvent::PolicyEvaluated {
                action_count,
                denied_count,
                ..
            } => {
                if *denied_count > 0 {
                    format!(
                        "policy evaluated: {}/{} actions denied",
                        denied_count, action_count
                    )
                } else {
                    format!("policy evaluated: {} actions allowed", action_count)
                }
            }
            LoopEvent::ToolsDispatched {
                tool_count,
                duration,
                ..
            } => {
                format!(
                    "tools dispatched: {} tools in {:.1}s",
                    tool_count,
                    duration.as_secs_f64()
                )
            }
            LoopEvent::ObservationsCollected {
                observation_count, ..
            } => {
                format!("collected {} observations", observation_count)
            }
            LoopEvent::Terminated { reason, .. } => {
                let reason_str = match reason {
                    TerminationReason::Completed => "completed",
                    TerminationReason::MaxIterations => "max iterations reached",
                    TerminationReason::MaxTokens => "token budget exhausted",
                    TerminationReason::Timeout => "timeout",
                    TerminationReason::PolicyDenial { reason } => reason.as_str(),
                    TerminationReason::Error { message } => message.as_str(),
                };
                format!("loop terminated: {}", reason_str)
            }
            LoopEvent::RecoveryTriggered {
                tool_name, error, ..
            } => {
                format!("recovery triggered for {}: {}", tool_name, error)
            }
            // Catch-all for feature-gated variants we don't link against.
            #[allow(unreachable_patterns)]
            _ => "phase transition".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl JournalWriter for ProgressJournalWriter {
    async fn append(&self, entry: JournalEntry) -> Result<(), JournalError> {
        self.sequence.fetch_add(1, Ordering::Relaxed);

        if let Some(tx) = &self.tx {
            let detail = Self::detail_for_event(&entry.event);
            let event = ProgressEvent::AgentIteration {
                name: self.package_name.clone(),
                iteration: entry.iteration,
                max_iterations: self.max_iterations,
                detail,
            };
            // Ignore send errors — no receivers is fine.
            let _ = tx.send(event);
        }

        Ok(())
    }

    async fn next_sequence(&self) -> u64 {
        self.sequence.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbi_runtime::reasoning::loop_types::LoopConfig;
    use symbi_runtime::types::AgentId;

    fn make_entry(iteration: u32, event: LoopEvent) -> JournalEntry {
        JournalEntry {
            sequence: 0,
            timestamp: chrono::Utc::now(),
            agent_id: AgentId::new(),
            iteration,
            event,
        }
    }

    #[tokio::test]
    async fn test_progress_journal_sends_events() {
        let (tx, mut rx) = broadcast::channel::<ProgressEvent>(16);
        let writer = ProgressJournalWriter::new("test-pkg".into(), 10, Some(tx));

        let entry = make_entry(
            0,
            LoopEvent::Started {
                agent_id: AgentId::new(),
                config: Box::new(LoopConfig::default()),
            },
        );

        writer.append(entry).await.unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            ProgressEvent::AgentIteration {
                name,
                iteration,
                max_iterations,
                detail,
            } => {
                assert_eq!(name, "test-pkg");
                assert_eq!(iteration, 0);
                assert_eq!(max_iterations, 10);
                assert!(detail.contains("starting"));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[tokio::test]
    async fn test_progress_journal_no_tx_is_fine() {
        let writer = ProgressJournalWriter::new("test-pkg".into(), 5, None);
        let entry = make_entry(
            1,
            LoopEvent::Started {
                agent_id: AgentId::new(),
                config: Box::new(LoopConfig::default()),
            },
        );
        // Should not panic.
        writer.append(entry).await.unwrap();
        assert_eq!(writer.next_sequence().await, 1);
    }
}
