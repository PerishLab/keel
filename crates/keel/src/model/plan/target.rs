use super::Resource;
use core::marker::PhantomData;

pub struct Probe<T: ?Sized>(PhantomData<fn() -> T>);

impl<T: ?Sized> Clone for Probe<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Probe<T> {}

impl<T: ?Sized> Probe<T> {
    pub const NEW: Self = Self(PhantomData);
}

pub trait Named {
    fn pick(self, fallback: &'static str) -> &'static str;
}

impl<T: Resource + ?Sized> Named for &Probe<T> {
    fn pick(self, _: &'static str) -> &'static str {
        T::name()
    }
}

pub trait Plain {
    fn pick(self, fallback: &'static str) -> &'static str;
}

impl<T: ?Sized> Plain for Probe<T> {
    fn pick(self, fallback: &'static str) -> &'static str {
        fallback
    }
}
