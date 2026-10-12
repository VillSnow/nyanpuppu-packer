use dioxus::prelude::*;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub struct Ctx<T: 'static, Tag: 'static>(Signal<T>, PhantomData<fn() -> Tag>);

impl<T: 'static, Tag: 'static> Ctx<T, Tag> {
    pub fn new(value: T) -> Self {
        Self(Signal::new(value), PhantomData)
    }
}

impl<T: 'static, Tag: 'static> Clone for Ctx<T, Tag> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static, Tag: 'static> Copy for Ctx<T, Tag> {}

impl<T: 'static, Tag: 'static> Deref for Ctx<T, Tag> {
    type Target = Signal<T>;
    fn deref(&self) -> &Signal<T> {
        &self.0
    }
}

impl<T: 'static, Tag: 'static> DerefMut for Ctx<T, Tag> {
    fn deref_mut(&mut self) -> &mut Signal<T> {
        &mut self.0
    }
}
