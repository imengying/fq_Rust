use crate::config::DeviceProfile;
use crate::fq::now_ms;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

#[derive(Clone)]
pub struct DevicePoolManager {
    inner: Arc<Mutex<DevicePoolState>>,
}

struct DevicePoolState {
    active_profile: DeviceProfile,
    profiles: Vec<DeviceProfile>,
    active_index: Option<usize>,
    rotate_cooldown_ms: u64,
    last_rotate_at_ms: i64,
}

impl DevicePoolManager {
    pub fn new(
        active_profile: DeviceProfile,
        profiles: Vec<DeviceProfile>,
        startup_name: Option<String>,
        rotate_cooldown_ms: u64,
    ) -> Self {
        let active_index = resolve_active_index(&active_profile, &profiles, startup_name.as_deref());
        let manager = Self {
            inner: Arc::new(Mutex::new(DevicePoolState {
                active_profile,
                profiles,
                active_index,
                rotate_cooldown_ms,
                last_rotate_at_ms: 0,
            })),
        };
        let current = manager.current_profile();
        info!(
            "device profile initialized: name={}, device_id={}, install_id={}",
            current.name,
            current.device.device_id,
            current.device.install_id
        );
        manager
    }

    pub fn current_profile(&self) -> DeviceProfile {
        self.inner
            .lock()
            .expect("device pool lock poisoned")
            .active_profile
            .clone()
    }

    pub fn rotate_if_allowed(&self, reason: &str) -> bool {
        let mut state = match self.inner.lock() {
            Ok(state) => state,
            Err(_) => return false,
        };

        if state.profiles.len() <= 1 {
            return false;
        }

        let now = now_ms();
        if state.rotate_cooldown_ms > 0
            && state.last_rotate_at_ms > 0
            && now - state.last_rotate_at_ms < state.rotate_cooldown_ms as i64
        {
            return false;
        }

        let next_index = match next_rotation_index(state.active_index, state.profiles.len()) {
            Some(index) => index,
            None => return false,
        };
        let next_profile = state.profiles[next_index].clone();
        let previous_name = state.active_profile.name.clone();
        state.active_profile = next_profile.clone();
        state.active_index = Some(next_index);
        state.last_rotate_at_ms = now;

        warn!(
            "rotated device profile: reason={}, from={}, to={}, device_id={}, install_id={}",
            reason,
            previous_name,
            next_profile.name,
            next_profile.device.device_id,
            next_profile.device.install_id
        );
        true
    }
}

fn resolve_active_index(
    active_profile: &DeviceProfile,
    profiles: &[DeviceProfile],
    startup_name: Option<&str>,
) -> Option<usize> {
    if profiles.is_empty() {
        return None;
    }

    if let Some(name) = active_profile_name(active_profile) {
        if let Some(index) = profiles
            .iter()
            .position(|profile| active_profile_name(profile) == Some(name))
        {
            return Some(index);
        }
    }

    if let Some(name) = startup_name.and_then(normalize_name) {
        if let Some(index) = profiles
            .iter()
            .position(|profile| active_profile_name(profile) == Some(name))
        {
            return Some(index);
        }
    }

    profiles.iter().position(|profile| profile == active_profile).or(Some(0))
}

fn next_rotation_index(current_index: Option<usize>, len: usize) -> Option<usize> {
    if len <= 1 {
        return None;
    }
    let current = current_index.unwrap_or(0);
    Some((current + 1) % len)
}

fn active_profile_name(profile: &DeviceProfile) -> Option<&str> {
    normalize_name(&profile.name)
}

fn normalize_name(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DeviceProfile, UpstreamDevice};

    #[test]
    fn rotates_to_next_profile() {
        let dev01 = profile("dev01", "device-1");
        let dev02 = profile("dev02", "device-2");
        let manager = DevicePoolManager::new(
            dev01.clone(),
            vec![dev01.clone(), dev02.clone()],
            None,
            0,
        );

        assert!(manager.rotate_if_allowed("TEST"));
        assert_eq!(manager.current_profile().name, "dev02");
    }

    #[test]
    fn respects_startup_name_for_index_tracking() {
        let dev01 = profile("dev01", "device-1");
        let dev02 = profile("dev02", "device-2");
        let manager = DevicePoolManager::new(dev02.clone(), vec![dev01, dev02], Some("dev02".to_string()), 0);

        assert!(manager.rotate_if_allowed("TEST"));
        assert_eq!(manager.current_profile().name, "dev01");
    }

    fn profile(name: &str, device_id: &str) -> DeviceProfile {
        DeviceProfile {
            name: name.to_string(),
            user_agent: "ua".to_string(),
            cookie: "install_id=1".to_string(),
            device: UpstreamDevice {
                device_id: device_id.to_string(),
                install_id: format!("install-{device_id}"),
                ..UpstreamDevice::default()
            },
        }
    }
}
