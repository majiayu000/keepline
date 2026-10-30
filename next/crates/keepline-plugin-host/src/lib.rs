//! Loads sidebar plugins into a sandbox and asks them for a sidebar description.
//!
//! Prototype ABI (see specs/plugins/plugin-system.md): the plugin exports `memory`,
//! `alloc(len) -> ptr`, `dealloc(ptr, cap)` and `render(ptr, len) -> ptr`; the reply at
//! the returned pointer is a 4-byte little-endian length followed by JSON.
use std::time::Duration;
use wasmtime::error::{Context, Result, bail, format_err};
use wasmtime::{Config, Instance, Memory, Store, TypedFunc};

/// How long one call may run before it is interrupted, in epoch ticks of `TICK`.
pub const TICK: Duration = Duration::from_millis(10);
const DEADLINE_TICKS: u64 = 5;
/// Linear memory a plugin may grow to.
const MEMORY_LIMIT: usize = 64 << 20;

pub fn engine() -> Result<Engine> {
    let mut config = Config::new();
    config.epoch_interruption(true);
    Engine::new(&config)
}

pub use wasmtime::{Engine, Module};

/// Compiles a module from its `.wasm` bytes for `Plugin::new`.
pub fn module(engine: &Engine, wasm: &[u8]) -> Result<Module> {
    Module::new(engine, wasm)
}

/// Compiles once; the bytes can be cached on disk and loaded with `Plugin::from_precompiled`.
pub fn precompile(engine: &Engine, wasm: &[u8]) -> Result<Vec<u8>> {
    engine.precompile_module(wasm)
}

struct Limits(wasmtime::StoreLimits);

pub struct Plugin {
    store: Store<Limits>,
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    dealloc: TypedFunc<(u32, u32), ()>,
    render: TypedFunc<(u32, u32), u32>,
    instance: Instance,
}

impl Plugin {
    pub fn new(engine: &Engine, module: &Module) -> Result<Self> {
        let limits = wasmtime::StoreLimitsBuilder::new()
            .memory_size(MEMORY_LIMIT)
            .build();
        let mut store = Store::new(engine, Limits(limits));
        store.limiter(|l| &mut l.0);
        store.set_epoch_deadline(DEADLINE_TICKS);
        // No imports: the plugin has no way to reach files, network or processes.
        let instance = Instance::new(&mut store, module, &[])?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("plugin exports no memory")?;
        Ok(Self {
            alloc: instance.get_typed_func(&mut store, "alloc")?,
            dealloc: instance.get_typed_func(&mut store, "dealloc")?,
            render: instance.get_typed_func(&mut store, "render")?,
            memory,
            store,
            instance,
        })
    }

    /// # Safety
    /// `bytes` must come from `precompile` with the same engine configuration.
    pub unsafe fn from_precompiled(engine: &Engine, bytes: &[u8]) -> Result<Self> {
        // SAFETY: forwarded to the caller.
        let module = unsafe { Module::deserialize(engine, bytes)? };
        Self::new(engine, &module)
    }

    /// One sidebar update: state JSON in, sidebar JSON out. Any trap, timeout or bad
    /// reply is an error, never a crash; the caller falls back to the built-in view.
    pub fn render(&mut self, state: &[u8]) -> Result<Vec<u8>> {
        self.store.set_epoch_deadline(DEADLINE_TICKS);
        let len = u32::try_from(state.len()).context("state too large")?;
        let input = self.alloc.call(&mut self.store, len)?;
        self.memory.write(&mut self.store, input as usize, state)?;
        let out = self.render.call(&mut self.store, (input, len))?;
        self.dealloc.call(&mut self.store, (input, len))?;
        let mut header = [0u8; 4];
        self.memory.read(&self.store, out as usize, &mut header)?;
        let n = u32::from_le_bytes(header) as usize;
        if n > MEMORY_LIMIT {
            bail!("reply of {n} bytes");
        }
        let mut reply = vec![0u8; n];
        self.memory
            .read(&self.store, out as usize + 4, &mut reply)?;
        self.dealloc.call(&mut self.store, (out, (n + 4) as u32))?;
        serde_json::from_slice::<serde_json::Value>(&reply)
            .map_err(|e| format_err!("plugin replied with invalid JSON: {e}"))?;
        Ok(reply)
    }

    /// Calls a no-argument export; used to check that a runaway plugin is interrupted.
    pub fn call_raw(&mut self, name: &str) -> Result<u32> {
        self.store.set_epoch_deadline(DEADLINE_TICKS);
        let f: TypedFunc<(), u32> = self.instance.get_typed_func(&mut self.store, name)?;
        f.call(&mut self.store, ())
    }
}

/// Advances the engine's epoch every `TICK`, so a call past its deadline traps.
pub fn start_ticker(engine: &Engine) {
    let engine = engine.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK);
            engine.increment_epoch();
        }
    });
}
