//! Windows devnode (PnP) lookups for HID interface paths.
//!
//! The string handling is pure and testable on its own; only the `CM_*` calls are
//! `#[cfg(windows)]`.

/// Deepest ancestor chain we walk looking for the composite USB device.
const MAX_PARENT_DEPTH: usize = 8;

/// Convert a hidapi interface path into the PnP device instance id.
///
/// `\\?\hid#vid_1b1c&pid_0c3f&mi_00#7&2a5c1f&0&0000#{4d1e55b2-...}`
/// becomes `HID\VID_1B1C&PID_0C3F&MI_00\7&2A5C1F&0&0000`.
///
/// Tolerant of case, a `\\?\` / `\\.\` / missing prefix and a missing interface-class GUID
/// suffix. Returns `None` when fewer than the three `enumerator#hardware-id#instance` segments exist.
pub fn interface_path_to_instance_id(path: &str) -> Option<String> {
    let trimmed = path.trim();
    let body = trimmed
        .strip_prefix(r"\\?\")
        .or_else(|| trimmed.strip_prefix(r"\\.\"))
        .or_else(|| trimmed.strip_prefix(r"\??\"))
        .unwrap_or(trimmed);

    let mut parts = body.split('#');
    let enumerator = parts.next()?;
    let hardware = parts.next()?;
    let instance = parts.next()?;
    if enumerator.is_empty() || hardware.is_empty() || instance.is_empty() {
        return None;
    }
    Some(format!("{enumerator}\\{hardware}\\{instance}").to_uppercase())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevnodeStatus {
    Started,
    /// The devnode has a problem (CM_PROB_* code).
    Problem(u32),
    NotStarted,
    /// The devnode could not be located (removed or never present).
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevnodeInfo {
    pub status: DevnodeStatus,
    /// Enumerator name (`USB`, `HID`, `BTHENUM`, ...).
    pub enumerator: Option<String>,
    pub parent_instance_id: Option<String>,
}

/// Whether a device can be opened now (the hotplug readiness check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    Problem(u32),
    NotStarted,
    Missing,
}

impl Readiness {
    pub fn is_ready(self) -> bool {
        self == Readiness::Ready
    }
}

impl From<DevnodeStatus> for Readiness {
    fn from(status: DevnodeStatus) -> Self {
        match status {
            DevnodeStatus::Started => Readiness::Ready,
            DevnodeStatus::Problem(code) => Readiness::Problem(code),
            DevnodeStatus::NotStarted => Readiness::NotStarted,
            DevnodeStatus::Missing => Readiness::Missing,
        }
    }
}

const DN_STARTED: u32 = 0x0000_0008;
const DN_HAS_PROBLEM: u32 = 0x0000_0400;

/// Map raw `CM_Get_DevNode_Status` output to a [`DevnodeStatus`].
pub fn status_from_flags(status: u32, problem: u32) -> DevnodeStatus {
    if status & DN_HAS_PROBLEM != 0 {
        DevnodeStatus::Problem(problem)
    } else if status & DN_STARTED != 0 {
        DevnodeStatus::Started
    } else {
        DevnodeStatus::NotStarted
    }
}

/// Walk ancestors (starting at `first_parent`) to the first devnode whose enumerator is USB
/// and whose id has no `&MI_` segment: the composite USB device, not a single interface.
///
/// `query` resolves an instance id to its [`DevnodeInfo`].
pub fn find_usb_ancestor(
    first_parent: Option<String>,
    mut query: impl FnMut(&str) -> Option<DevnodeInfo>,
) -> Option<String> {
    let mut current = first_parent;
    for _ in 0..MAX_PARENT_DEPTH {
        let id = current?;
        let info = query(&id)?;
        let is_usb = info
            .enumerator
            .as_deref()
            .is_some_and(|e| e.eq_ignore_ascii_case("USB"));
        if is_usb && !id.to_ascii_uppercase().contains("&MI_") {
            return Some(id);
        }
        current = info.parent_instance_id;
    }
    None
}

#[cfg(windows)]
pub use win::{query_devnode, readiness, usb_device_instance_id};

#[cfg(windows)]
mod win {
    use super::*;
    use anyhow::{Result, bail};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_DEVNODE_STATUS_FLAGS, CM_DRP_ENUMERATOR_NAME, CM_Get_DevNode_Registry_PropertyW, CM_PROB, CM_Get_DevNode_Status,
        CM_Get_Device_IDW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW,
        CR_SUCCESS,
    };
    use windows::core::PCWSTR;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn device_id(devinst: u32) -> Option<String> {
        let mut buf = [0u16; 512];
        // SAFETY: `buf` is a valid writable slice; the API writes at most buf.len() UTF-16 units.
        let cr = unsafe { CM_Get_Device_IDW(devinst, &mut buf, 0) };
        if cr != CR_SUCCESS {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    fn enumerator_name(devinst: u32) -> Option<String> {
        let mut buf = [0u16; 128];
        let mut bytes = std::mem::size_of_val(&buf) as u32;
        // SAFETY: `buf` is valid for `bytes` bytes and `bytes` is a valid in/out length.
        let cr = unsafe {
            CM_Get_DevNode_Registry_PropertyW(
                devinst,
                CM_DRP_ENUMERATOR_NAME,
                None,
                Some(buf.as_mut_ptr().cast()),
                &mut bytes,
                0,
            )
        };
        if cr != CR_SUCCESS {
            return None;
        }
        let units = ((bytes as usize) / 2).min(buf.len());
        let len = buf[..units].iter().position(|&c| c == 0).unwrap_or(units);
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    /// Look up the devnode for `instance_id`. A devnode that cannot be located is reported as
    /// [`DevnodeStatus::Missing`], not an error; `Err` is reserved for an unusable argument.
    pub fn query_devnode(instance_id: &str) -> Result<DevnodeInfo> {
        if instance_id.is_empty() || instance_id.contains('\0') {
            bail!("invalid device instance id");
        }
        let id = wide(instance_id);
        let mut devinst = 0u32;
        // SAFETY: `id` is NUL-terminated and outlives the call; `devinst` is a valid out pointer.
        let cr = unsafe {
            CM_Locate_DevNodeW(&mut devinst, PCWSTR(id.as_ptr()), CM_LOCATE_DEVNODE_NORMAL)
        };
        if cr != CR_SUCCESS {
            return Ok(DevnodeInfo {
                status: DevnodeStatus::Missing,
                enumerator: None,
                parent_instance_id: None,
            });
        }

        let (mut flags, mut problem) = (CM_DEVNODE_STATUS_FLAGS(0), CM_PROB(0));
        // SAFETY: both out pointers are valid for the duration of the call.
        let status = if unsafe { CM_Get_DevNode_Status(&mut flags, &mut problem, devinst, 0) }
            == CR_SUCCESS
        {
            status_from_flags(flags.0, problem.0)
        } else {
            DevnodeStatus::Missing
        };

        let mut parent = 0u32;
        // SAFETY: `parent` is a valid out pointer.
        let parent_instance_id = if unsafe { CM_Get_Parent(&mut parent, devinst, 0) } == CR_SUCCESS
        {
            device_id(parent)
        } else {
            None
        };

        Ok(DevnodeInfo {
            status,
            enumerator: enumerator_name(devinst),
            parent_instance_id,
        })
    }

    /// Instance id of the composite USB device that owns `instance_id`.
    pub fn usb_device_instance_id(instance_id: &str) -> Option<String> {
        let start = query_devnode(instance_id).ok()?;
        find_usb_ancestor(start.parent_instance_id, |id| query_devnode(id).ok())
    }

    /// Readiness of the devnode behind a hidapi interface path.
    pub fn readiness(path: &str) -> Readiness {
        let Some(id) = interface_path_to_instance_id(path) else {
            return Readiness::Missing;
        };
        query_devnode(&id)
            .map(|info| info.status.into())
            .unwrap_or(Readiness::Missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUID: &str = "{4d1e55b2-f16f-11cf-88cb-001111000030}";

    #[test]
    fn converts_lowercase_path() {
        let p = format!(r"\\?\hid#vid_1b1c&pid_0c3f&mi_00#7&2a5c1f&0&0000#{GUID}");
        assert_eq!(
            interface_path_to_instance_id(&p).as_deref(),
            Some(r"HID\VID_1B1C&PID_0C3F&MI_00\7&2A5C1F&0&0000")
        );
    }

    #[test]
    fn converts_uppercase_path_and_dot_prefix() {
        let p = format!(r"\\.\HID#VID_1B1C&PID_1C1F#6&1A2B3C4D&0&0000#{GUID}");
        assert_eq!(
            interface_path_to_instance_id(&p).as_deref(),
            Some(r"HID\VID_1B1C&PID_1C1F\6&1A2B3C4D&0&0000")
        );
    }

    #[test]
    fn keeps_collection_suffix() {
        let p = format!(r"\\?\hid#vid_1b1c&pid_0c3f&mi_01&col01#8&3f2e1d&0&0000#{GUID}");
        assert_eq!(
            interface_path_to_instance_id(&p).as_deref(),
            Some(r"HID\VID_1B1C&PID_0C3F&MI_01&COL01\8&3F2E1D&0&0000")
        );
    }

    #[test]
    fn tolerates_missing_prefix_and_guid() {
        assert_eq!(
            interface_path_to_instance_id("hid#vid_1b1c&pid_0c3f&mi_00#7&2a5c1f&0&0000").as_deref(),
            Some(r"HID\VID_1B1C&PID_0C3F&MI_00\7&2A5C1F&0&0000")
        );
    }

    #[test]
    fn converts_bluetooth_path() {
        let p = r"\\?\hid#{00001124-0000-1000-8000-00805f9b34fb}_vid&0002_pid&1234#9&aa&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
        assert_eq!(
            interface_path_to_instance_id(p).as_deref(),
            Some(r"HID\{00001124-0000-1000-8000-00805F9B34FB}_VID&0002_PID&1234\9&AA&0&0000")
        );
    }

    #[test]
    fn rejects_malformed_paths() {
        for p in ["", r"\\?\", "hid", r"\\?\hid#only", r"\\?\hid##x#{g}", "garbage/no/hashes"] {
            assert_eq!(interface_path_to_instance_id(p), None, "{p:?}");
        }
    }

    #[test]
    fn status_flag_mapping() {
        assert_eq!(status_from_flags(DN_STARTED, 0), DevnodeStatus::Started);
        assert_eq!(
            status_from_flags(DN_HAS_PROBLEM | DN_STARTED, 10),
            DevnodeStatus::Problem(10)
        );
        assert_eq!(status_from_flags(0, 0), DevnodeStatus::NotStarted);
    }

    #[test]
    fn readiness_from_status() {
        assert!(Readiness::from(DevnodeStatus::Started).is_ready());
        assert_eq!(Readiness::from(DevnodeStatus::Problem(10)), Readiness::Problem(10));
        assert!(!Readiness::from(DevnodeStatus::NotStarted).is_ready());
        assert!(!Readiness::from(DevnodeStatus::Missing).is_ready());
    }

    fn node(enumerator: &str, parent: Option<&str>) -> DevnodeInfo {
        DevnodeInfo {
            status: DevnodeStatus::Started,
            enumerator: Some(enumerator.into()),
            parent_instance_id: parent.map(String::from),
        }
    }

    #[test]
    fn usb_ancestor_skips_interface_node() {
        let q = |id: &str| match id {
            r"USB\VID_1B1C&PID_0C3F&MI_00\6&1&0&0000" => {
                Some(node("USB", Some(r"USB\VID_1B1C&PID_0C3F\SERIAL")))
            }
            r"USB\VID_1B1C&PID_0C3F\SERIAL" => Some(node("USB", Some(r"USB\ROOT_HUB30\4&0"))),
            _ => None,
        };
        assert_eq!(
            find_usb_ancestor(Some(r"USB\VID_1B1C&PID_0C3F&MI_00\6&1&0&0000".into()), q).as_deref(),
            Some(r"USB\VID_1B1C&PID_0C3F\SERIAL")
        );
    }

    #[test]
    fn usb_ancestor_none_for_non_usb_chain() {
        let q = |id: &str| match id {
            r"BTHENUM\DEV" => Some(node("BTHENUM", Some(r"ACPI\ROOT"))),
            r"ACPI\ROOT" => Some(node("ACPI", None)),
            _ => None,
        };
        assert_eq!(find_usb_ancestor(Some(r"BTHENUM\DEV".into()), q), None);
        assert_eq!(find_usb_ancestor(None, |_| None), None);
    }

    #[test]
    fn usb_ancestor_walk_is_bounded() {
        // A cycle must terminate.
        let q = |_: &str| Some(node("PCI", Some("LOOP")));
        assert_eq!(find_usb_ancestor(Some("LOOP".into()), q), None);
    }
}
