use std::fmt::Display;

#[derive(Debug,thiserror::Error)]
pub enum Error {
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),
    NoSuchAdapter,
}


impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
       match self {
           Error::CreateSurfaceError(e) => write!(f,"{}",e),
              Error::NoSuchAdapter => write!(f,"No such adapter"),
       }
    }
}