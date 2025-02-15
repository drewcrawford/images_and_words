#[derive(Debug)]
pub struct Producer<T>(T);
#[derive(Debug)]
pub struct Receiver<T>(T);
#[derive(Debug)]
pub struct ProducerWriteGuard<T>(T);

impl<T> std::ops::Deref for ProducerWriteGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl<T> std::ops::DerefMut for ProducerWriteGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        todo!()
    }
}

#[derive(Debug)]
pub struct ProducerReadGuard<T>(T);

impl<T> std::ops::Deref for ProducerReadGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}
#[derive(Debug,Clone)]
pub struct ReceiverReadGuard<T>(T);

impl<T> std::ops::Deref for ReceiverReadGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl<T> Producer<T> {
    pub async fn borrow_write(& mut self) -> ProducerWriteGuard<T>
    {
        todo!()
    }
    pub async fn submit(&mut self, _guard: ProducerWriteGuard<T>){
        todo!()
    }
    pub fn borrow_last_read<'a>(&self) -> ProducerReadGuard<T>  {
        todo!()
    }
}

impl<Delivery> Receiver<Delivery> {
    /**
    Gets new items from the Producer.  if items are not ready, returns the last, cached item.
    */
    pub fn receive(&mut self) -> ReceiverReadGuard<Delivery> {
        todo!()
    }
}

pub fn multibuffer<Product,Delivery>(_products: Vec<Product>) -> (Producer<Product>,Receiver<Delivery>) {
    todo!()
}