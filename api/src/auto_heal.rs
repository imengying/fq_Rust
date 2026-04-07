use crate::fq::now_ms;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AutoHealManager {
    inner: Arc<Mutex<AutoHealState>>,
}

struct AutoHealState {
    enabled: bool,
    error_threshold: usize,
    window_ms: u64,
    cooldown_ms: u64,
    failure_times: VecDeque<i64>,
    last_heal_at_ms: i64,
}

impl AutoHealManager {
    pub fn new(enabled: bool, error_threshold: usize, window_ms: u64, cooldown_ms: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AutoHealState {
                enabled,
                error_threshold: error_threshold.max(1),
                window_ms,
                cooldown_ms,
                failure_times: VecDeque::new(),
                last_heal_at_ms: 0,
            })),
        }
    }

    pub fn record_success(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.failure_times.clear();
        }
    }

    pub fn record_failure_and_should_heal(&self) -> bool {
        let now = now_ms();
        let mut state = match self.inner.lock() {
            Ok(state) => state,
            Err(_) => return false,
        };
        if !state.enabled {
            return false;
        }

        let window_ms = state.window_ms;
        trim_window(&mut state.failure_times, window_ms, now);
        state.failure_times.push_back(now);
        if state.failure_times.len() < state.error_threshold {
            return false;
        }

        if state.cooldown_ms > 0
            && state.last_heal_at_ms > 0
            && now - state.last_heal_at_ms < state.cooldown_ms as i64
        {
            return false;
        }

        state.last_heal_at_ms = now;
        state.failure_times.clear();
        true
    }
}

fn trim_window(values: &mut VecDeque<i64>, window_ms: u64, now: i64) {
    if window_ms == 0 {
        values.clear();
        return;
    }
    while let Some(front) = values.front() {
        if now - *front > window_ms as i64 {
            values.pop_front();
        } else {
            break;
        }
    }
}
