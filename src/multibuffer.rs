use std::marker::PhantomData;

pub struct Multibuffer<T> {
    t: PhantomData<T>
}