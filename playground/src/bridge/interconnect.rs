#![allow(clippy::module_name_repetitions)]

use color_eyre::eyre;
use std::ffi::CString;
use std::os::raw::c_char;
use std::path::Path;

#[cxx::bridge]
mod default {
    unsafe extern "C++" {
        include!("playground/src/bridge.hpp");

        type c_void;

        type IntersimConfig;
        #[must_use]
        fn new_intersim_config() -> UniquePtr<IntersimConfig>;
        #[must_use]
        fn ParseFile(self: Pin<&mut IntersimConfig>, filename: &CxxString) -> i32;
        #[must_use]
        fn GetInt(self: &IntersimConfig, field: &CxxString) -> i32;
        #[must_use]
        fn GetStr<'a, 'b>(self: &'a IntersimConfig, field: &'b CxxString) -> &'a CxxString;
        #[must_use]
        fn GetFloat(self: &IntersimConfig, field: &CxxString) -> f64;

        type BoxInterconnect;
        #[must_use]
        unsafe fn new_box_interconnect(config_file: *const c_char) -> UniquePtr<BoxInterconnect>;

        #[must_use]
        fn HasBuffer(self: &BoxInterconnect, deviceID: u32, size: u32) -> bool;
        fn Advance(self: Pin<&mut BoxInterconnect>);
        #[must_use]
        fn Busy(self: &BoxInterconnect) -> bool;
        #[must_use]
        unsafe fn Pop(self: Pin<&mut BoxInterconnect>, deviceID: u32) -> *mut c_void;
        unsafe fn Push(
            self: Pin<&mut BoxInterconnect>,
            input_deviceID: u32,
            output_deviceID: u32,
            data: *mut c_void,
            size: u32,
        );
        fn Init(self: Pin<&mut BoxInterconnect>);
        fn CreateInterconnect(self: Pin<&mut BoxInterconnect>, n_shader: u32, n_mem: u32);
        fn DisplayMap(self: &BoxInterconnect, dim: u32, count: u32);
        fn GetNumNodes(self: &BoxInterconnect) -> u32;
        fn GetNumMemories(self: &BoxInterconnect) -> u32;
        fn GetNumShaders(self: &BoxInterconnect) -> u32;
        #[must_use]
        fn GetConfig(self: &BoxInterconnect) -> SharedPtr<IntersimConfig>;

        type InterconnectInterface;
        #[must_use]
        unsafe fn new_interconnect_interface(
            config_file: *const c_char,
        ) -> UniquePtr<InterconnectInterface>;

        #[must_use]
        fn HasBuffer(self: &InterconnectInterface, deviceID: u32, size: u32) -> bool;
        fn Advance(self: Pin<&mut InterconnectInterface>);
        #[must_use]
        fn Busy(self: &InterconnectInterface) -> bool;
        #[must_use]
        unsafe fn Pop(self: Pin<&mut InterconnectInterface>, deviceID: u32) -> *mut c_void;
        unsafe fn Push(
            self: Pin<&mut InterconnectInterface>,
            input_deviceID: u32,
            output_deviceID: u32,
            data: *mut c_void,
            size: u32,
        );
        fn Init(self: Pin<&mut InterconnectInterface>);
        fn CreateInterconnect(self: Pin<&mut InterconnectInterface>, n_shader: u32, n_mem: u32);
        fn DisplayMap(self: &InterconnectInterface, dim: u32, count: u32);
        fn GetNumNodes(self: &InterconnectInterface) -> u32;
        fn GetNumMemories(self: &InterconnectInterface) -> u32;
        fn GetNumShaders(self: &InterconnectInterface) -> u32;
        #[must_use]
        fn GetConfig(self: &InterconnectInterface) -> SharedPtr<IntersimConfig>;
    }
}

pub struct InterconnectInterface(cxx::UniquePtr<default::InterconnectInterface>);
pub struct BoxInterconnect(cxx::UniquePtr<default::BoxInterconnect>);

impl InterconnectInterface {
    #[must_use]
    pub fn new(config_file: &Path, num_clusters: u32, num_mem_sub_partitions: u32) -> Self {
        let config_file = config_file.canonicalize().unwrap();
        let config_file = CString::new(&*config_file.to_string_lossy()).unwrap();
        let mut interconn =
            Self(unsafe { default::new_interconnect_interface(config_file.as_ptr()) });
        interconn.create_interconnect(num_clusters, num_mem_sub_partitions);
        interconn.init();
        interconn
    }
}

impl BoxInterconnect {
    #[must_use]
    pub fn new(config_file: &Path, num_clusters: u32, num_mem_sub_partitions: u32) -> Self {
        let config_file = config_file.canonicalize().unwrap();
        let config_file = CString::new(&*config_file.to_string_lossy()).unwrap();
        let mut interconn = Self(unsafe { default::new_box_interconnect(config_file.as_ptr()) });
        interconn.create_interconnect(num_clusters, num_mem_sub_partitions);
        interconn.init();
        interconn
    }
}

pub trait BridgedInterconnect {
    fn init(&mut self);
    fn create_interconnect(&mut self, num_clusters: u32, num_mem_sub_partitions: u32);
    fn num_nodes(&self) -> u32;
    fn num_shaders(&self) -> u32;
    fn num_memories(&self) -> u32;
    fn advance(&mut self);
    fn has_buffer(&mut self, node: u32, size: u32) -> bool;
    unsafe fn push(
        &mut self,
        src_node: u32,
        dest_node: u32,
        value: *mut default::c_void,
        size: u32,
    );
    fn pop(&mut self, node: u32) -> *mut default::c_void;
}

impl BridgedInterconnect for InterconnectInterface {
    fn init(&mut self) {
        self.0.pin_mut().Init();
    }
    fn create_interconnect(&mut self, num_clusters: u32, num_mem_sub_partitions: u32) {
        self.0
            .pin_mut()
            .CreateInterconnect(num_clusters, num_mem_sub_partitions);
    }
    #[must_use]
    fn num_nodes(&self) -> u32 {
        self.0.GetNumNodes()
    }
    fn num_shaders(&self) -> u32 {
        #[must_use]
        self.0.GetNumShaders()
    }
    #[must_use]
    fn num_memories(&self) -> u32 {
        self.0.GetNumMemories()
    }
    fn advance(&mut self) {
        self.0.pin_mut().Advance();
    }
    #[must_use]
    fn has_buffer(&mut self, node: u32, size: u32) -> bool {
        self.0.HasBuffer(node, size)
    }
    unsafe fn push(
        &mut self,
        src_node: u32,
        dest_node: u32,
        value: *mut default::c_void,
        size: u32,
    ) {
        self.0.pin_mut().Push(src_node, dest_node, value, size);
    }
    #[must_use]
    fn pop(&mut self, node: u32) -> *mut default::c_void {
        unsafe { self.0.pin_mut().Pop(node) }
    }
}

impl BridgedInterconnect for BoxInterconnect {
    fn init(&mut self) {
        self.0.pin_mut().Init();
    }
    fn create_interconnect(&mut self, num_clusters: u32, num_mem_sub_partitions: u32) {
        self.0
            .pin_mut()
            .CreateInterconnect(num_clusters, num_mem_sub_partitions);
    }
    #[must_use]
    fn num_nodes(&self) -> u32 {
        self.0.GetNumNodes()
    }
    #[must_use]
    fn num_shaders(&self) -> u32 {
        self.0.GetNumShaders()
    }
    #[must_use]
    fn num_memories(&self) -> u32 {
        self.0.GetNumMemories()
    }
    fn advance(&mut self) {
        self.0.pin_mut().Advance();
    }
    #[must_use]
    fn has_buffer(&mut self, node: u32, size: u32) -> bool {
        self.0.HasBuffer(node, size)
    }
    unsafe fn push(
        &mut self,
        src_node: u32,
        dest_node: u32,
        value: *mut default::c_void,
        size: u32,
    ) {
        self.0.pin_mut().Push(src_node, dest_node, value, size);
    }
    #[must_use]
    fn pop(&mut self, node: u32) -> *mut default::c_void {
        unsafe { self.0.pin_mut().Pop(node) }
    }
}

pub struct Interconnect<T, I> {
    inner: I,
    phantom: std::marker::PhantomData<T>,
}

impl<T, I> Interconnect<T, I>
where
    I: BridgedInterconnect,
{
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            phantom: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn num_nodes(&self) -> u32 {
        self.inner.num_nodes()
    }

    #[must_use]
    pub fn num_shaders(&self) -> u32 {
        self.inner.num_shaders()
    }

    #[must_use]
    pub fn num_memories(&self) -> u32 {
        self.inner.num_memories()
    }

    pub fn advance(&mut self) {
        self.inner.advance();
    }

    pub fn must_pop(&mut self, node: u32, limit: Option<u16>) -> eyre::Result<(u16, Box<T>)> {
        let limit = limit.unwrap_or(u16::MAX);
        assert!(limit > 0);
        for cycle in 0..limit {
            if let Some(data) = self.pop(node) {
                return Ok((cycle, data));
            }
            self.advance();
        }
        Err(eyre::eyre!(
            "timeout waiting for message after {} cycles",
            limit
        ))
    }

    #[must_use]
    pub fn pop(&mut self, node: u32) -> Option<Box<T>> {
        let value = unsafe { self.inner.pop(node) };
        if value.is_null() {
            None
        } else {
            let value: Box<T> = unsafe { Box::from_raw(value.cast::<T>()) };
            Some(value)
        }
    }

    pub fn push(&mut self, src_node: u32, dest_node: u32, value: Box<T>) {
        let mut value: &mut T = Box::leak(value);
        assert!(self.inner.has_buffer(src_node, 8));
        unsafe {
            self.inner.push(
                src_node,
                dest_node,
                (value as *mut T).cast::<default::c_void>(),
                u32::try_from(std::mem::size_of::<T>()).unwrap(),
            );
        }
    }
}

pub struct IntersimConfig(cxx::UniquePtr<default::IntersimConfig>);

impl std::fmt::Display for IntersimConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("IntersimConfig").finish()
    }
}

impl std::fmt::Debug for IntersimConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("IntersimConfig").finish()
    }
}

impl Default for IntersimConfig {
    #[must_use]
    fn default() -> Self {
        Self::new()
    }
}

impl IntersimConfig {
    #[must_use]
    pub fn new() -> Self {
        Self(default::new_intersim_config())
    }

    pub fn from_file(path: &Path) -> eyre::Result<Self> {
        let mut config = Self(default::new_intersim_config());
        config.parse_file(path)?;
        Ok(config)
    }

    fn parse_file(&mut self, path: &Path) -> eyre::Result<()> {
        let config_file = path.canonicalize()?.to_string_lossy().to_string();
        cxx::let_cxx_string!(config_file = config_file);
        self.0.pin_mut().ParseFile(&config_file);
        Ok(())
    }

    #[must_use]
    pub fn get_int(&self, field: impl AsRef<str>) -> i32 {
        cxx::let_cxx_string!(field = field.as_ref());
        self.0.GetInt(&field)
    }

    #[must_use]
    pub fn get_string(&self, field: impl AsRef<str>) -> String {
        cxx::let_cxx_string!(field = field.as_ref());
        self.0.GetStr(&field).to_string_lossy().to_string()
    }

    #[must_use]
    pub fn get_bool(&self, field: impl AsRef<str>) -> bool {
        self.get_int(field) != 0
    }
}
