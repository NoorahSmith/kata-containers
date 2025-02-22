// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::HypervisorState;
use crate::device::DeviceType;
use crate::VmmState;
use anyhow::Result;
use async_trait::async_trait;
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use kata_types::config::hypervisor::HYPERVISOR_NAME_CH;
use persist::sandbox_persist::Persist;
use std::os::unix::net::UnixStream;
use tokio::process::Child;
use tokio::sync::watch::{channel, Receiver, Sender};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct CloudHypervisorInner {
    pub(crate) state: VmmState,
    pub(crate) id: String,

    pub(crate) api_socket: Option<UnixStream>,
    pub(crate) extra_args: Option<Vec<String>>,

    pub(crate) config: Option<HypervisorConfig>,

    pub(crate) process: Option<Child>,
    pub(crate) pid: Option<u32>,

    pub(crate) timeout_secs: i32,

    pub(crate) netns: Option<String>,

    // Sandbox-specific directory
    pub(crate) vm_path: String,

    // Hypervisor runtime directory
    pub(crate) run_dir: String,

    // Subdirectory of vm_path.
    pub(crate) jailer_root: String,

    /// List of devices that will be added to the VM once it boots
    pub(crate) pending_devices: Vec<DeviceType>,

    pub(crate) _capabilities: Capabilities,

    pub(crate) shutdown_tx: Option<Sender<bool>>,
    pub(crate) shutdown_rx: Option<Receiver<bool>>,
    pub(crate) tasks: Option<Vec<JoinHandle<Result<()>>>>,
}

const CH_DEFAULT_TIMEOUT_SECS: u32 = 10;

impl CloudHypervisorInner {
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.set(
            CapabilityBits::BlockDeviceSupport
                | CapabilityBits::BlockDeviceHotplugSupport
                | CapabilityBits::FsSharingSupport,
        );

        let (tx, rx) = channel(true);

        Self {
            api_socket: None,
            extra_args: None,

            process: None,
            pid: None,

            config: None,
            state: VmmState::NotReady,
            timeout_secs: CH_DEFAULT_TIMEOUT_SECS as i32,
            id: String::default(),
            jailer_root: String::default(),
            vm_path: String::default(),
            run_dir: String::default(),
            netns: None,
            pending_devices: vec![],
            _capabilities: capabilities,
            shutdown_tx: Some(tx),
            shutdown_rx: Some(rx),
            tasks: None,
        }
    }

    pub fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        self.config = Some(config);
    }

    pub fn hypervisor_config(&self) -> HypervisorConfig {
        self.config.clone().unwrap_or_default()
    }
}

impl Default for CloudHypervisorInner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Persist for CloudHypervisorInner {
    type State = HypervisorState;
    type ConstructorArgs = ();

    // Return a state object that will be saved by the caller.
    async fn save(&self) -> Result<Self::State> {
        Ok(HypervisorState {
            hypervisor_type: HYPERVISOR_NAME_CH.to_string(),
            id: self.id.clone(),
            vm_path: self.vm_path.clone(),
            jailed: false,
            jailer_root: String::default(),
            netns: None,
            config: self.hypervisor_config(),
            run_dir: self.run_dir.clone(),
            cached_block_devices: Default::default(),
            ..Default::default()
        })
    }

    // Set the hypervisor state to the specified state
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let ch = Self {
            config: Some(hypervisor_state.config),
            state: VmmState::NotReady,
            id: hypervisor_state.id,
            vm_path: hypervisor_state.vm_path,
            run_dir: hypervisor_state.run_dir,

            ..Default::default()
        };

        Ok(ch)
    }
}
