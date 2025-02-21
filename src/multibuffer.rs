use std::marker::PhantomData;
use std::sync::Mutex;
use crate::bindings::resource_tracking::{CPUReadGuard, CPUWriteGuard, GPUGuard, ResourceTracker};
use crate::bindings::resource_tracking::sealed::Mappable;

pub struct Multibuffer<T> {
    //right now, not really a multibuffer!
    t: ResourceTracker<T>,
    wake_list: Mutex<Vec<r#continue::Sender<()>>>
}

impl<T> Multibuffer<T> {
    pub fn new(element: T) -> Self {
        let tracker = ResourceTracker::new(element, || {
            todo!()
        });
        Multibuffer {
            t: tracker,
            wake_list: Mutex::new(Vec::new())
        }
    }

    pub async fn access_read(&self) -> CPUReadGuard<T> where T: Mappable {
        loop {
            //insert first
            let (s,f) = r#continue::continuation();
            self.wake_list.lock().unwrap().push(s);
            //then check
            match self.t.cpu_read().await {
                Ok(guard) => return guard,
                Err(_) => f.await
            }
        }
    }

    pub async fn access_write(&self) -> CPUWriteGuard<T> where T: Mappable {
        loop {
            //insert first
            let (s, f) = r#continue::continuation();
            self.wake_list.lock().unwrap().push(s);
            //then check
            match self.t.cpu_write().await {
                Ok(guard) => return guard,
                Err(_) => f.await
            }
        }
    }

    pub fn access_gpu(&self) -> GPUGuard<T> where T: Mappable {
        self.t.gpu().expect("multibuffer access_gpu")
    }
}