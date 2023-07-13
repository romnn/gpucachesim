use crate::bindings;

super::extern_type!(bindings::cache_config_params, "cache_config_params");

#[cxx::bridge]
mod default {
    unsafe extern "C++" {
        include!("playground/src/bridge.hpp");

        type cache_config_params = crate::bindings::cache_config_params;
        type cache_config;
        #[must_use] fn new_cache_config(config: cache_config_params) -> UniquePtr<cache_config>;

        #[must_use] fn disabled(self: &cache_config) -> bool;
        #[must_use] fn is_streaming(self: &cache_config) -> bool;
    }
}

pub use default::*;
