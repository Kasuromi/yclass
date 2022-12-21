use crate::config::YClassConfig;
use libloading::Library;
use memflex::external::{MemoryRegion, OwnedProcess};
use std::fs;

pub struct ManagedExtension {
    #[allow(dead_code)]
    lib: Library,

    attach: extern "C" fn(u32) -> u32,
    read: extern "C" fn(usize, *mut u8, usize) -> u32,
    write: extern "C" fn(usize, *const u8, usize) -> u32,
    can_read: extern "C" fn(usize) -> bool,
    detach: extern "C" fn(),
}

impl Drop for ManagedExtension {
    fn drop(&mut self) {
        (self.detach)();
    }
}

enum AttachedProcess {
    Internal {
        proc: OwnedProcess,
        cached_regions: Vec<MemoryRegion>,
    },
    Managed(u32),
}

pub struct ProcessManager {
    plugin: Option<ManagedExtension>,
    attached: Option<AttachedProcess>,
}

impl ProcessManager {
    pub fn new(config: &YClassConfig) -> eyre::Result<Self> {
        let (path, modified) = (
            config
                .plugin_path
                .clone()
                .unwrap_or_else(|| "plugin.ycpl".into()),
            config.plugin_path.is_some(),
        );

        let metadata = fs::metadata(&path);
        let plugin = if metadata.is_ok() {
            let lib = unsafe { Library::new(&path)? };
            let attach = unsafe { *lib.get::<extern "C" fn(u32) -> u32>(b"yc_attach")? };
            let read =
                unsafe { *lib.get::<extern "C" fn(usize, *mut u8, usize) -> u32>(b"yc_read")? };
            let write =
                unsafe { *lib.get::<extern "C" fn(usize, *const u8, usize) -> u32>(b"yc_write")? };
            let can_read = unsafe { *lib.get::<extern "C" fn(usize) -> bool>(b"yc_can_read")? };
            let detach = unsafe { *lib.get::<extern "C" fn()>(b"yc_detach")? };

            Some(ManagedExtension {
                lib,
                attach,
                read,
                write,
                can_read,
                detach,
            })
        } else if modified {
            return Err(eyre::eyre!("Failed to create plugin. Path doesn't exists"));
        } else {
            None
        };

        Ok(Self {
            attached: None,
            plugin,
        })
    }

    pub fn is_attached(&self) -> bool {
        self.attached.is_some()
    }

    pub fn attach(&mut self, pid: u32) -> eyre::Result<()> {
        if let Some(ref pl) = self.plugin {
            let code = (pl.attach)(pid);
            if code != 0 {
                return Err(eyre::eyre!("Plugin attach error. Error code: {code}"));
            }

            self.attached = Some(AttachedProcess::Managed(pid));
        } else {
            #[cfg(unix)]
            let proc = memflex::external::find_process_by_id(pid)?;
            #[cfg(windows)]
            let proc = {
                use memflex::types::win::{
                    PROCESS_QUERY_INFORMATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
                };

                memflex::external::open_process_by_id(
                    pid,
                    false,
                    PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_QUERY_INFORMATION,
                )?
            };

            let maps = proc.maps()?;
            self.attached = Some(AttachedProcess::Internal {
                cached_regions: maps,
                proc,
            });
        }

        Ok(())
    }

    pub fn detach(&mut self) {
        if let Some(ref pl) = self.plugin {
            (pl.detach)();
        }

        self.attached = None;
    }

    pub fn read(&self, address: usize, buf: &mut [u8]) {
        // TODO: Error handling

        if let Some(ref pl) = self.plugin {
            assert!((pl.read)(address, buf.as_mut_ptr(), buf.len()) == 0);
        } else {
            let Some(AttachedProcess::Internal { ref proc, .. }) = self.attached else {
                unreachable!()
            };

            proc.read_buf(address, buf).unwrap();
        }
    }

    pub fn write(&self, address: usize, buf: &[u8]) {
        // TODO: Error handling

        if let Some(ref pl) = self.plugin {
            assert!((pl.write)(address, buf.as_ptr(), buf.len()) == 0);
        } else {
            let Some(AttachedProcess::Internal { ref proc, .. }) = self.attached else {
                unreachable!()
            };

            proc.write_buf(address, buf).unwrap();
        }
    }

    pub fn id(&self) -> u32 {
        // TODO: Error handling

        match self.attached {
            Some(AttachedProcess::Managed(id)) => id,
            Some(AttachedProcess::Internal { ref proc, .. }) => proc.id(),
            _ => unreachable!(),
        }
    }

    pub fn can_read(&self, address: usize) -> bool {
        if let Some(ref pl) = self.plugin {
            (pl.can_read)(address)
        } else {
            let Some(AttachedProcess::Internal { ref cached_regions, .. }) = self.attached else {
                unreachable!()
            };

            cached_regions
                .iter()
                .any(|r| r.from <= address && r.to >= address && r.prot.read())
        }
    }

    pub fn name(&self) -> eyre::Result<String> {
        if self.plugin.is_some() {
            Ok("[MANAGED]".into())
        } else {
            let Some(AttachedProcess::Internal { ref proc, .. }) = self.attached else {
                unreachable!()
            };

            Ok(proc.name()?)
        }
    }
}
